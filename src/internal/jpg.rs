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
        -T[(32 - k) as usize]
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
