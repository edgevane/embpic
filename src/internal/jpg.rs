//! Baseline JPEG decoder (sequential 8-bit, 1/3 components).
//! Transport: mmap via raw syscalls. Codec: DQT/SOF0/DHT/SOS,
//! Huffman + dequant + IDCT + YCbCr->RGB, subsampling via nearest.

extern crate alloc;
use alloc::vec::Vec;

use crate::image::Image;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnsupportedPlatform,
    Open(i32),
    Stat(i32),
    Empty,
    Map(i32),
    NotJpeg,
    Truncated,
    Unsupported,
    BadHuffman,
    Encode,
    Write(i32),
}

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40,
    48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61,
    54, 47, 55, 62, 63,
];

#[derive(Clone)]
struct HTable {
    codes: Vec<(u16, u8, u8)>, // (code, len, symbol)
}

impl HTable {
    fn build(counts: [u8; 16], symbols: Vec<u8>) -> Self {
        let mut codes = Vec::new();
        let mut code: u16 = 0;
        let mut k = 0;
        for (i, &n) in counts.iter().enumerate() {
            for _ in 0..n {
                if k < symbols.len() {
                    codes.push((code, (i + 1) as u8, symbols[k]));
                    k += 1;
                }
                code += 1;
            }
            code <<= 1;
        }
        Self { codes }
    }
    fn decode(&self, br: &mut BitReader) -> Result<u8, Error> {
        let mut code: u16 = 0;
        for len in 1..=16u8 {
            let b = br.bit().ok_or(Error::Truncated)?;
            code = (code << 1) | (b as u16);
            for &(c, l, s) in &self.codes {
                if l == len && c == code {
                    return Ok(s);
                }
            }
        }
        Err(Error::BadHuffman)
    }
}

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    nbits: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, buf: 0, nbits: 0 }
    }
    fn next_byte(&mut self) -> Option<u8> {
        if self.pos >= self.data.len() {
            return None;
        }
        let b = self.data[self.pos];
        self.pos += 1;
        if b == 0xFF {
            // stuffed zero or marker; RSTn (D0-D7) = resync, else end
            if self.pos < self.data.len() {
                let m = self.data[self.pos];
                if m == 0x00 {
                    self.pos += 1;
                    return Some(0xFF);
                }
                if (0xD0..=0xD7).contains(&m) {
                    self.pos += 1;
                    self.buf = 0;
                    self.nbits = 0;
                    return self.next_byte();
                }
                return None;
            }
            return None;
        }
        Some(b)
    }
    fn bit(&mut self) -> Option<u8> {
        if self.nbits == 0 {
            let b = self.next_byte()?;
            self.buf = b as u32;
            self.nbits = 8;
        }
        self.nbits -= 1;
        Some(((self.buf >> self.nbits) & 1) as u8)
    }
    fn bits(&mut self, n: u8) -> Option<u16> {
        let mut v = 0u16;
        for _ in 0..n {
            v = (v << 1) | self.bit()? as u16;
        }
        Some(v)
    }
}

fn extend(v: u16, t: u8) -> i32 {
    if t == 0 {
        return 0;
    }
    let vt = 1 << (t - 1);
    let v = v as i32;
    if v < vt {
        v - (1 << t) + 1
    } else {
        v
    }
}

/// cos(k*pi/16) lookup — no libm needed in no_std.
fn cos16(k: u32) -> f32 {
    const T: [f32; 16] = [
        1.0, 0.98078528, 0.92387953, 0.83146961, 0.70710678, 0.55557023,
        0.38268343, 0.19509032, 0.0, -0.19509032, -0.38268343, -0.55557023,
        -0.70710678, -0.83146961, -0.92387953, -0.98078528,
    ];
    let k = k % 32;
    if k < 16 {
        T[k as usize]
    } else {
        // cos(pi + t) = -cos(t)
        -T[(k - 16) as usize]
    }
}

fn idct(block: &mut [f32; 64]) {
    let mut tmp = [0f32; 64];
    for y in 0..8 {
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                for v in 0..8 {
                    let cu = if u == 0 { 0.70710678 } else { 1.0 };
                    let cv = if v == 0 { 0.70710678 } else { 1.0 };
                    s += cu
                        * cv
                        * block[v * 8 + u]
                        * cos16(((2 * x + 1) * u) as u32)
                        * cos16(((2 * y + 1) * v) as u32);
                }
            }
            tmp[y * 8 + x] = 0.25 * s;
        }
    }
    *block = tmp;
}

struct Comp {
    id: u8,
    hs: u8,
    vs: u8,
    tq: u8,
    dc: u8,
    ac: u8,
    plane: Vec<f32>,
    cw: u32,
    ch: u32,
}

pub fn decode(data: &[u8]) -> Result<Image, Error> {
    if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(Error::NotJpeg);
    }
    let mut quant = [[1u16; 64]; 4];
    let mut dc_t: [Option<HTable>; 4] = [None, None, None, None];
    let mut ac_t: [Option<HTable>; 4] = [None, None, None, None];
    let mut comps: Vec<Comp> = Vec::new();
    let mut width = 0u32;
    let mut height = 0u32;
    let mut i = 2usize;
    let mut scan_start = 0usize;

    while i + 2 <= data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = data[i + 1];
        if m == 0xD9 {
            break;
        }
        if m == 0xDA {
            // SOS
            if i + 4 > data.len() {
                return Err(Error::Truncated);
            }
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            let ns = data[i + 4] as usize;
            for k in 0..ns {
                let cs = data[i + 5 + k * 2];
                let sel = data[i + 6 + k * 2];
                for c in comps.iter_mut() {
                    if c.id == cs {
                        c.dc = sel >> 4;
                        c.ac = sel & 0xF;
                    }
                }
            }
            scan_start = i + 2 + len;
            break;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if len < 2 || i + 2 + len > data.len() {
            return Err(Error::Truncated);
        }
        let body = &data[i + 4..i + 2 + len];
        match m {
            0xDB => {
                let mut k = 0;
                while k < body.len() {
                    let info = body[k];
                    let tq = (info & 0x0F) as usize;
                    if tq > 3 {
                        return Err(Error::Truncated);
                    }
                    if info >> 4 == 0 {
                        if k + 65 > body.len() {
                            return Err(Error::Truncated);
                        }
                        // DQT bytes arrive in zigzag order → scatter to natural.
                        for j in 0..64 {
                            quant[tq][ZIGZAG[j]] = body[k + 1 + j] as u16;
                        }
                        k += 65;
                    } else {
                        if k + 129 > body.len() {
                            return Err(Error::Truncated);
                        }
                        for j in 0..64 {
                            let o = k + 1 + j * 2;
                            quant[tq][ZIGZAG[j]] =
                                u16::from_be_bytes([body[o], body[o + 1]]);
                        }
                        k += 129;
                    }
                }
            }
            0xC0 => {
                height = u16::from_be_bytes([body[1], body[2]]) as u32;
                width = u16::from_be_bytes([body[3], body[4]]) as u32;
                let nc = body[5] as usize;
                comps.clear();
                for k in 0..nc {
                    let o = 6 + k * 3;
                    comps.push(Comp {
                        id: body[o],
                        hs: body[o + 1] >> 4,
                        vs: body[o + 1] & 0xF,
                        tq: body[o + 2],
                        dc: 0,
                        ac: 0,
                        plane: Vec::new(),
                        cw: 0,
                        ch: 0,
                    });
                }
            }
            0xC2 => return Err(Error::Unsupported),
            0xC4 => {
                let mut k = 0;
                while k + 17 <= body.len() {
                    let info = body[k];
                    let mut counts = [0u8; 16];
                    counts.copy_from_slice(&body[k + 1..k + 17]);
                    let total: usize =
                        counts.iter().map(|&c| c as usize).sum();
                    if k + 17 + total > body.len() {
                        return Err(Error::Truncated);
                    }
                    let syms = body[k + 17..k + 17 + total].to_vec();
                    let t = HTable::build(counts, syms);
                    let idx = (info & 3) as usize;
                    if info >> 4 == 0 {
                        dc_t[idx] = Some(t);
                    } else {
                        ac_t[idx] = Some(t);
                    }
                    k += 17 + total;
                }
            }
            _ => {}
        }
        i += 2 + len;
    }

    if width == 0 || height == 0 || comps.is_empty() {
        return Err(Error::Truncated);
    }
    if comps.len() != 1 && comps.len() != 3 {
        return Err(Error::Unsupported);
    }
    let hmax = comps.iter().map(|c| c.hs).max().unwrap_or(1).max(1);
    let vmax = comps.iter().map(|c| c.vs).max().unwrap_or(1).max(1);
    let mcu_w = hmax as u32 * 8;
    let mcu_h = vmax as u32 * 8;
    let mcus_x = width.div_ceil(mcu_w);
    let mcus_y = height.div_ceil(mcu_h);
    for c in comps.iter_mut() {
        c.cw = mcus_x * c.hs as u32 * 8;
        c.ch = mcus_y * c.vs as u32 * 8;
        c.plane = alloc::vec![0.0; (c.cw * c.ch) as usize];
    }

    let mut br = BitReader::new(&data[scan_start..]);
    let mut prev_dc = [0i32; 4];
    let mut block = [0f32; 64];
    for my in 0..mcus_y {
        for mx in 0..mcus_x {
            for ci in 0..comps.len() {
                let (hh, vv, tq, dc_i, ac_i, cw) = {
                    let c = &comps[ci];
                    (c.hs, c.vs, c.tq, c.dc, c.ac, c.cw)
                };
                let dc_tab =
                    dc_t[dc_i as usize].as_ref().ok_or(Error::BadHuffman)?;
                let ac_tab =
                    ac_t[ac_i as usize].as_ref().ok_or(Error::BadHuffman)?;
                for by in 0..vv {
                    for bx in 0..hh {
                        let mut zz = [0i32; 64];
                        let t = dc_tab.decode(&mut br)?;
                        let diff = extend(
                            br.bits(t).ok_or(Error::Truncated)?, t,
                        );
                        prev_dc[ci] += diff;
                        zz[0] = prev_dc[ci];
                        let mut k = 1usize;
                        while k < 64 {
                            let rs =
                                ac_tab.decode(&mut br)?;
                            if rs == 0x00 {
                                break;
                            }
                            if rs == 0xF0 {
                                k += 16;
                                continue;
                            }
                            let run = (rs >> 4) as usize;
                            let s = rs & 0xF;
                            k += run;
                            if k >= 64 {
                                return Err(Error::BadHuffman);
                            }
                            zz[ZIGZAG[k]] = extend(
                                br.bits(s).ok_or(Error::Truncated)?, s,
                            );
                            k += 1;
                        }
                        for (j, b) in block.iter_mut().enumerate() {
                            *b = zz[j] as f32
                                * quant[tq as usize][j] as f32;
                        }
                        idct(&mut block);
                        let ox = mx * hh as u32 * 8 + bx as u32 * 8;
                        let oy = my * vv as u32 * 8 + by as u32 * 8;
                        let ch = comps[ci].ch;
                        let plane = &mut comps[ci].plane;
                        for y in 0..8u32 {
                            for x in 0..8u32 {
                                let px = ox + x;
                                let py = oy + y;
                                if px < cw && py < ch {
                                    plane[(py * cw + px) as usize] =
                                        block[(y * 8 + x) as usize] + 128.0;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // upsample nearest + YCbCr->RGB
    let mut rgb = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut p = [0u8; 3];
            for (ci, c) in comps.iter().enumerate() {
                let sx = (x * c.cw / width).min(c.cw - 1);
                let sy = (y * c.ch / height).min(c.ch - 1);
                p[ci] = c.plane[(sy * c.cw + sx) as usize]
                    .clamp(0.0, 255.0) as u8;
            }
            let (r, g, b) = if comps.len() == 1 {
                (p[0], p[0], p[0])
            } else {
                let yv = p[0] as f32;
                let cb = p[1] as f32 - 128.0;
                let cr = p[2] as f32 - 128.0;
                (
                    (yv + 1.402 * cr).clamp(0.0, 255.0) as u8,
                    (yv - 0.344136 * cb - 0.714136 * cr).clamp(0.0, 255.0)
                        as u8,
                    (yv + 1.772 * cb).clamp(0.0, 255.0) as u8,
                )
            };
            rgb.extend_from_slice(&[r, g, b]);
        }
    }
    Ok(Image::from_rgb(width, height, rgb))
}

// --- file transport (mmap/read) ---

pub struct MappedFile {
    pub ptr: *const u8,
    len: usize,
}

impl MappedFile {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let _ = crate::internal::platform::linux_x86::munmap(
                self.ptr as *mut u8,
                self.len,
            );
        }
    }
}

unsafe impl Send for MappedFile {}
unsafe impl Sync for MappedFile {}

fn to_cstring(path: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(path.len() + 1);
    v.extend_from_slice(path.as_bytes());
    v.push(0);
    v
}

pub fn mmap(path: &str) -> Result<MappedFile, Error> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        use crate::internal::platform::linux_x86 as p;
        let c = to_cstring(path);
        let fd = p::open(c.as_ptr(), p::O_RDONLY, 0).map_err(Error::Open)?;
        let size = p::file_size(fd).map_err(|e| {
            let _ = p::close(fd);
            Error::Stat(e)
        })?;
        if size == 0 {
            let _ = p::close(fd);
            return Err(Error::Empty);
        }
        let ptr = p::mmap(size, p::PROT_READ, p::MAP_PRIVATE, fd, 0).map_err(|e| {
            let _ = p::close(fd);
            Error::Map(e)
        })?;
        let _ = p::close(fd);
        Ok(MappedFile { ptr, len: size })
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = path;
        Err(Error::UnsupportedPlatform)
    }
}

pub fn load(path: &str) -> Result<Image, Error> {
    let m = mmap(path)?;
    decode(m.as_bytes())
}

// --- baseline encoder (8-bit sequential, 3 comps, 4:4:4) ---

/// Luma quant table, natural order (IJG Annex K, quality 50).
const Q_LUMA: [u16; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13,
    16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56,
    68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103,
    121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// Chroma quant table, natural order (IJG Annex K, quality 50).
const Q_CHROMA: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26,
    56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

fn fdct(block: &[f32; 64]) -> [i32; 64] {
    let mut out = [0i32; 64];
    for v in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                for x in 0..8 {
                    s += block[y * 8 + x]
                        * cos16(((2 * x + 1) * u) as u32)
                        * cos16(((2 * y + 1) * v) as u32);
                }
            }
            let cu = if u == 0 { 0.70710678 } else { 1.0 };
            let cv = if v == 0 { 0.70710678 } else { 1.0 };
            // round-half-away without libm (no_std).
            let x = 0.25 * cu * cv * s;
            out[v * 8 + u] =
                if x >= 0.0 { (x + 0.5) as i32 } else { (x - 0.5) as i32 };
        }
    }
    out
}

fn category(v: i32) -> u8 {
    let mut a = if v < 0 { -v } else { v };
    let mut n = 0;
    while a > 0 {
        n += 1;
        a >>= 1;
    }
    n
}

/// Canonical encode map built from counts+symbols: symbol -> (code, len).
struct EncTable {
    map: [(u16, u8); 256],
}

impl EncTable {
    fn build(counts: [u8; 16], symbols: &[u8]) -> Self {
        let mut map = [(0u16, 0u8); 256];
        let mut code: u16 = 0;
        let mut k = 0;
        for (i, &n) in counts.iter().enumerate() {
            for _ in 0..n {
                if k < symbols.len() {
                    map[symbols[k] as usize] = (code, (i + 1) as u8);
                    k += 1;
                }
                code += 1;
            }
            code <<= 1;
        }
        Self { map }
    }
}

fn dc_symbols() -> Vec<u8> {
    (0u8..12).collect()
}

fn ac_symbols() -> Vec<u8> {
    // EOB, ZRL, then all (run 0..=15, size 1..=10) combos.
    let mut v = Vec::with_capacity(162);
    v.push(0x00);
    v.push(0xF0);
    for run in 0..16u8 {
        for size in 1..=10u8 {
            let rs = (run << 4) | size;
            if rs != 0x00 && rs != 0xF0 {
                v.push(rs);
            }
        }
    }
    v
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    nbits: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self { out: Vec::new(), acc: 0, nbits: 0 }
    }
    fn bits(&mut self, code: u16, len: u8) {
        self.acc = (self.acc << len) | (code as u32);
        self.nbits += len;
        while self.nbits >= 8 {
            self.nbits -= 8;
            let b = (self.acc >> self.nbits) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0x00);
            }
            self.acc &= (1 << self.nbits) - 1;
        }
    }
    fn amplitude(&mut self, v: i32, size: u8) {
        if size == 0 {
            return;
        }
        let bits = if v >= 0 { v as u16 } else { (v + (1 << size) - 1) as u16 };
        self.bits(bits, size);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            let b = (self.acc << (8 - self.nbits)) as u8 | ((1 << (8 - self.nbits)) - 1);
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0x00);
            }
        }
        self.out
    }
}

fn be16(v: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn encode_image(img: &Image) -> Result<Vec<u8>, Error> {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 || w > 65500 || h > 65500 {
        return Err(Error::Encode);
    }
    let rgb = img.as_rgb();

    let dc_syms = dc_symbols();
    let ac_syms = ac_symbols();
    let dc_counts = [0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut ac_counts = [0u8; 16];
    ac_counts[7] = 162;
    let dc_enc = EncTable::build(dc_counts, &dc_syms);
    let ac_enc = EncTable::build(ac_counts, &ac_syms);

    let mut out = Vec::new();
    // SOI
    out.extend_from_slice(&[0xFF, 0xD8]);
    // APP0 JFIF
    out.extend_from_slice(&[
        0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01,
        0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
    ]);
    // DQT: table 0 = luma, table 1 = chroma (zigzag order on wire)
    out.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x84, 0x00]);
    for j in 0..64 {
        out.push(Q_LUMA[ZIGZAG[j]] as u8);
    }
    out.push(0x01);
    for j in 0..64 {
        out.push(Q_CHROMA[ZIGZAG[j]] as u8);
    }
    // SOF0
    out.extend_from_slice(&[0xFF, 0xC0]);
    be16(8 + 3 * 3, &mut out);
    out.push(8);
    be16(h as u16, &mut out);
    be16(w as u16, &mut out);
    out.push(3);
    out.extend_from_slice(&[0x01, 0x11, 0x00]); // Y
    out.extend_from_slice(&[0x02, 0x11, 0x01]); // Cb
    out.extend_from_slice(&[0x03, 0x11, 0x01]); // Cr
    // DHT: 4 tables (class,id): DC0, AC0, DC1, AC1
    for &(class, id, counts, syms) in [
        (0u8, 0u8, dc_counts, dc_syms.as_slice()),
        (1u8, 0u8, ac_counts, ac_syms.as_slice()),
        (0u8, 1u8, dc_counts, dc_syms.as_slice()),
        (1u8, 1u8, ac_counts, ac_syms.as_slice()),
    ]
    .iter()
    {
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        out.extend_from_slice(&[0xFF, 0xC4]);
        be16((2 + 1 + 16 + total) as u16, &mut out);
        out.push((class << 4) | id);
        out.extend_from_slice(&counts);
        out.extend_from_slice(&syms[..total]);
    }
    // SOS
    out.extend_from_slice(&[
        0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11,
        0x00, 0x3F, 0x00,
    ]);

    // Scan: 4:4:4, one 8x8 block per component per MCU position.
    let bw = w.div_ceil(8);
    let bh = h.div_ceil(8);
    let mut bwtr = BitWriter::new();
    let mut prev_dc = [0i32; 3];
    let mut fblock = [0f32; 64];
    for my in 0..bh {
        for mx in 0..bw {
            for ci in 0..3 {
                for y in 0..8 {
                    for x in 0..8 {
                        let px = (mx * 8 + x as u32).min(w - 1);
                        let py = (my * 8 + y as u32).min(h - 1);
                        let i = ((py * w + px) * 3) as usize;
                        let (r, g, b) =
                            (rgb[i] as f32, rgb[i + 1] as f32, rgb[i + 2] as f32);
                        fblock[y * 8 + x] = match ci {
                            0 => 0.299 * r + 0.587 * g + 0.114 * b - 128.0,
                            1 => -0.168736 * r - 0.331264 * g + 0.5 * b,
                            _ => 0.5 * r - 0.418688 * g - 0.081312 * b,
                        };
                    }
                }
                let d = fdct(&fblock);
                let q = if ci == 0 { &Q_LUMA } else { &Q_CHROMA };
                let mut zz = [0i32; 64];
                for (j, z) in zz.iter_mut().enumerate() {
                    let v = d[j];
                    *z = if v >= 0 {
                        ((v + (q[j] as i32 / 2)) / q[j] as i32) as i32
                    } else {
                        -(((-v) + (q[j] as i32 / 2)) / q[j] as i32)
                    };
                }
                let is_luma = ci == 0;
                let dc_t = &dc_enc;
                let ac_t = &ac_enc;
                let _ = is_luma;
                let diff = zz[0] - prev_dc[ci];
                prev_dc[ci] = zz[0];
                let s = category(diff);
                let (code, len) = dc_t.map[s as usize];
                bwtr.bits(code, len);
                bwtr.amplitude(diff, s);
                // AC in zigzag order
                let mut zero_run = 0;
                let mut k = 1usize;
                while k < 64 {
                    let v = zz[ZIGZAG[k]];
                    if v == 0 {
                        zero_run += 1;
                        k += 1;
                        continue;
                    }
                    while zero_run > 15 {
                        let (code, len) = ac_t.map[0xF0];
                        bwtr.bits(code, len);
                        zero_run -= 16;
                    }
                    let s = category(v);
                    let rs = ((zero_run << 4) | s as i32) as u8;
                    let (code, len) = ac_t.map[rs as usize];
                    bwtr.bits(code, len);
                    bwtr.amplitude(v, s);
                    zero_run = 0;
                    k += 1;
                }
                if zero_run > 0 {
                    let (code, len) = ac_t.map[0x00];
                    bwtr.bits(code, len);
                }
            }
        }
    }
    out.extend_from_slice(&bwtr.finish());
    out.extend_from_slice(&[0xFF, 0xD9]);
    Ok(out)
}

/// Encode + write file via raw write() loop. Linux x86_64 only.
pub fn save(img: &Image, path: &str) -> Result<(), Error> {
    let bytes = encode_image(img)?;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        use crate::internal::platform::linux_x86 as p;
        let c = to_cstring(path);
        let fd = p::open(
            c.as_ptr(),
            p::O_WRONLY | p::O_CREAT | p::O_TRUNC,
            0o644,
        )
        .map_err(Error::Open)?;
        let r = p::write_all(fd, &bytes).map_err(Error::Write);
        let _ = p::close(fd);
        r
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (img, path, bytes);
        Err(Error::UnsupportedPlatform)
    }
}
