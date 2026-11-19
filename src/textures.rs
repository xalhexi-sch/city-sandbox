//! All textures are generated in code (no asset files). Everything is mip-mapped and
//! uses a repeating, anisotropic sampler so facades and roads stay clean at distance.

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub type Rgb = [f32; 3];

pub fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// Smooth value noise that tiles with the given lattice periods.
pub fn tile_noise(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let g = |ix: i32, iy: i32| hash(ix.rem_euclid(px), iy.rem_euclid(py), seed);
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = g(ix, iy);
    let b = g(ix + 1, iy);
    let c = g(ix, iy + 1);
    let d = g(ix + 1, iy + 1);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Tiling fractal noise in 0..1 for a w*h texture, base lattice spacing `s` pixels.
pub fn fbm(x: i32, y: i32, w: i32, h: i32, s: i32, seed: u32) -> f32 {
    let mut amp = 0.5;
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut sp = s;
    for o in 0..3 {
        let px = (w / sp).max(1);
        let py = (h / sp).max(1);
        sum += amp * tile_noise(x as f32 / sp as f32, y as f32 / sp as f32, px, py, seed + o);
        norm += amp;
        amp *= 0.5;
        sp = (sp / 2).max(1);
    }
    sum / norm
}

pub fn mul(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgb>,
}

impl Canvas {
    pub fn new(w: i32, h: i32, c: Rgb) -> Self {
        Self {
            w,
            h,
            px: vec![c; (w * h) as usize],
        }
    }
    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        self.px[(y.clamp(0, self.h - 1) * self.w + x.clamp(0, self.w - 1)) as usize]
    }
    pub fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, c);
            }
        }
    }
    pub fn blend(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb, a: f32) {
        for y in y0..y1 {
            for x in x0..x1 {
                if x >= 0 && y >= 0 && x < self.w && y < self.h {
                    let o = self.get(x, y);
                    self.set(x, y, mix(o, c, a));
                }
            }
        }
    }
    pub fn fill_with(&mut self, f: impl Fn(i32, i32) -> Rgb) {
        for y in 0..self.h {
            for x in 0..self.w {
                self.px[(y * self.w + x) as usize] = f(x, y);
            }
        }
    }

    pub fn into_image(self) -> Image {
        let (w, h) = (self.w as u32, self.h as u32);
        let mut level: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
        for c in &self.px {
            level.push((c[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            level.push((c[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            level.push((c[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            level.push(255);
        }
        let mut all = level.clone();
        let (mut cw, mut ch) = (w, h);
        let mut levels = 1u32;
        while cw > 1 || ch > 1 {
            let nw = (cw / 2).max(1);
            let nh = (ch / 2).max(1);
            let mut next = vec![0u8; (nw * nh * 4) as usize];
            for y in 0..nh {
                for x in 0..nw {
                    for c in 0..4u32 {
                        let mut s = 0u32;
                        for (dx, dy) in [(0u32, 0u32), (1, 0), (0, 1), (1, 1)] {
                            let sx = (x * 2 + dx).min(cw - 1);
                            let sy = (y * 2 + dy).min(ch - 1);
                            s += level[((sy * cw + sx) * 4 + c) as usize] as u32;
                        }
                        next[((y * nw + x) * 4 + c) as usize] = (s / 4) as u8;
                    }
                }
            }
            all.extend_from_slice(&next);
            level = next;
            cw = nw;
            ch = nh;
            levels += 1;
        }
        let first = all[..(w * h * 4) as usize].to_vec();
        let mut img = Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            first,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        img.texture_descriptor.mip_level_count = levels;
        img.data = Some(all);
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 8,
            ..default()
        });
        img
    }
}

// ------------------------------------------------------------ ground tex ---

pub fn grass() -> Image {
    let (w, h) = (128, 128);
    let mut c = Canvas::new(w, h, [0.2, 0.4, 0.15]);
    c.fill_with(|x, y| {
        let n = fbm(x, y, w, h, 32, 3);
        let blade = hash(x, y, 9);
        let base = mix([0.14, 0.31, 0.11], [0.27, 0.46, 0.17], n);
        mul(base, 0.85 + 0.3 * blade)
    });
    c.into_image()
}

pub fn sidewalk() -> Image {
    // 128 px = 2 m slab
    let (w, h) = (128, 128);
    let mut c = Canvas::new(w, h, [0.6, 0.6, 0.58]);
    c.fill_with(|x, y| {
        let n = fbm(x, y, w, h, 32, 11);
        let s = hash(x, y, 4);
        let base = mix([0.55, 0.55, 0.53], [0.68, 0.67, 0.64], n);
        mul(base, 0.94 + 0.12 * s)
    });
    for i in 0..w {
        for t in 0..3 {
            c.set(i, t, [0.34, 0.34, 0.33]);
            c.set(t, i, [0.34, 0.34, 0.33]);
        }
    }
    c.into_image()
}

pub fn curb_stone() -> Image {
    let (w, h) = (64, 64);
    let mut c = Canvas::new(w, h, [0.5, 0.5, 0.5]);
    c.fill_with(|x, y| mul([0.52, 0.52, 0.5], 0.9 + 0.2 * hash(x, y, 5)));
    c.into_image()
}

pub fn gravel() -> Image {
    let (w, h) = (64, 64);
    let mut c = Canvas::new(w, h, [0.3, 0.3, 0.3]);
    c.fill_with(|x, y| mul([0.34, 0.33, 0.32], 0.7 + 0.6 * hash(x, y, 6)));
    c.into_image()
}

fn asphalt_px(x: i32, y: i32, w: i32, h: i32, seed: u32) -> Rgb {
    let n = fbm(x, y, w, h, 64, seed);
    let grain = hash(x, y, seed + 40);
    let mut k = 0.085 + 0.045 * n + 0.04 * grain;
    // occasional bright aggregate flecks
    if hash(x, y, seed + 77) > 0.985 {
        k += 0.08;
    }
    [k, k, k * 1.03]
}

/// 16 m wide, 36 m long road segment (16 px per metre). u = across, v = along.
/// Lane for "+" travel is at u = 0.25 (stop line at v ~ 1), "-" travel at u = 0.75 (stop line at v ~ 0).
pub fn road_segment() -> Image {
    let (w, h) = (256, 576);
    let mut c = Canvas::new(w, h, [0.1, 0.1, 0.1]);
    c.fill_with(|x, y| asphalt_px(x, y, w, h, 20));
    // worn tyre tracks in each lane
    for lane in [64, 192] {
        for off in [-13, 13] {
            for y in 0..h {
                let wob = (tile_noise(y as f32 / 40.0, 0.0, h / 40, 1, 5) - 0.5) * 6.0;
                for x in -6..=6 {
                    let a = 1.0 - (x as f32 / 6.0).abs();
                    let px = lane + off + x + wob as i32;
                    let o = c.get(px, y);
                    c.set(px, y, mul(o, 1.0 - 0.18 * a));
                }
            }
        }
    }
    // patches / cracks
    for i in 0..10 {
        let cx = (hash(i, 1, 31) * w as f32) as i32;
        let cy = (hash(i, 2, 31) * h as f32) as i32;
        let (pw, ph) = ((hash(i, 3, 31) * 40.0) as i32 + 12, (hash(i, 4, 31) * 60.0) as i32 + 20);
        c.blend(cx, cy, cx + pw, cy + ph, [0.07, 0.07, 0.075], 0.45);
    }
    let yellow = [0.82, 0.62, 0.08];
    let white = [0.8, 0.8, 0.78];
    // double yellow centre line
    c.rect(124, 0, 127, h, yellow);
    c.rect(129, 0, 132, h, yellow);
    // white edge lines
    c.rect(8, 0, 11, h, white);
    c.rect(245, 0, 248, h, white);
    // stop lines, 0.5 m before the intersection
    c.rect(22, h - 14, 118, h - 6, white); // "+" lane
    c.rect(138, 6, 234, 14, white); // "-" lane
    // wear on the paint
    for y in 0..h {
        for x in 0..w {
            let p = c.get(x, y);
            if p[0] > 0.5 || (p[0] > 0.4 && p[2] < 0.2) {
                let wear = hash(x, y, 90);
                if wear > 0.7 {
                    c.set(x, y, mul(p, 0.55 + 0.4 * (1.0 - wear)));
                }
            }
        }
    }
    c.into_image()
}

/// 16 x 16 m intersection with zebra crossings.
pub fn intersection() -> Image {
    let (w, h) = (256, 256);
    let mut c = Canvas::new(w, h, [0.1, 0.1, 0.1]);
    c.fill_with(|x, y| asphalt_px(x, y, w, h, 24));
    let white = [0.82, 0.82, 0.8];
    // crossings: 1.0 m .. 3.0 m inside each edge, bars 0.5 m wide
    for k in 0..28 {
        let a = 56 + k * 16;
        if a + 8 > 200 {
            break;
        }
        // north & south edges: bars run along z
        c.rect(a, 16, a + 8, 48, white);
        c.rect(a, 208, a + 8, 240, white);
        // west & east edges: bars run along x
        c.rect(16, a, 48, a + 8, white);
        c.rect(208, a, 240, a + 8, white);
    }
    for y in 0..h {
        for x in 0..w {
            let p = c.get(x, y);
            if p[0] > 0.5 && hash(x, y, 91) > 0.72 {
                c.set(x, y, mul(p, 0.65));
            }
        }
    }
    c.into_image()
}

// --------------------------------------------------------------- facades ---

pub const FAC_CELLS_X: i32 = 8;
pub const FAC_CELLS_Y: i32 = 12;
const CW: i32 = 48;
const CH: i32 = 52;

pub struct FacadeSpec {
    pub wall: Rgb,
    pub wall_alt: Rgb,
    pub brick: bool,
    pub frame: Rgb,
    pub glass_top: Rgb,
    pub glass_bot: Rgb,
    pub spandrel: Option<Rgb>,
    pub win: (f32, f32, f32, f32), // x0,x1,y0,y1 as cell fractions
    pub mullion: bool,
    pub transom: bool,
    pub sill: bool,
    pub piers: bool,
    pub balcony: bool,
    pub lit: f32,
    pub roughness: f32,
}

pub const FACADE_KINDS: usize = 8;

pub fn facade_spec(kind: usize) -> FacadeSpec {
    let base = FacadeSpec {
        wall: [0.6, 0.6, 0.6],
        wall_alt: [0.55, 0.55, 0.55],
        brick: false,
        frame: [0.85, 0.85, 0.8],
        glass_top: [0.30, 0.40, 0.50],
        glass_bot: [0.10, 0.13, 0.17],
        spandrel: None,
        win: (0.22, 0.78, 0.2, 0.78),
        mullion: true,
        transom: false,
        sill: true,
        piers: false,
        balcony: false,
        lit: 0.35,
        roughness: 0.75,
    };
    match kind % FACADE_KINDS {
        0 => FacadeSpec {
            wall: [0.50, 0.24, 0.17],
            wall_alt: [0.42, 0.19, 0.14],
            brick: true,
            ..base
        },
        1 => FacadeSpec {
            wall: [0.72, 0.66, 0.54],
            wall_alt: [0.66, 0.6, 0.5],
            frame: [0.25, 0.26, 0.28],
            win: (0.12, 0.88, 0.22, 0.82),
            mullion: true,
            lit: 0.4,
            ..base
        },
        2 => FacadeSpec {
            wall: [0.12, 0.18, 0.23],
            wall_alt: [0.1, 0.15, 0.2],
            frame: [0.42, 0.46, 0.5],
            glass_top: [0.45, 0.65, 0.8],
            glass_bot: [0.12, 0.25, 0.38],
            spandrel: Some([0.14, 0.2, 0.26]),
            win: (0.0, 1.0, 0.0, 0.84),
            mullion: true,
            sill: false,
            lit: 0.3,
            roughness: 0.18,
            ..base
        },
        3 => FacadeSpec {
            wall: [0.62, 0.62, 0.6],
            wall_alt: [0.56, 0.56, 0.54],
            frame: [0.14, 0.14, 0.15],
            win: (0.3, 0.7, 0.1, 0.9),
            mullion: false,
            sill: false,
            lit: 0.3,
            roughness: 0.85,
            ..base
        },
        4 => FacadeSpec {
            wall: [0.07, 0.07, 0.08],
            wall_alt: [0.06, 0.06, 0.07],
            frame: [0.1, 0.1, 0.11],
            glass_top: [0.2, 0.25, 0.3],
            glass_bot: [0.05, 0.06, 0.09],
            spandrel: Some([0.05, 0.05, 0.06]),
            win: (0.0, 1.0, 0.0, 0.85),
            mullion: true,
            sill: false,
            lit: 0.45,
            roughness: 0.12,
            ..base
        },
        5 => FacadeSpec {
            wall: [0.82, 0.76, 0.62],
            wall_alt: [0.77, 0.7, 0.58],
            frame: [0.95, 0.95, 0.92],
            win: (0.25, 0.75, 0.16, 0.66),
            balcony: true,
            lit: 0.4,
            ..base
        },
        6 => FacadeSpec {
            wall: [0.68, 0.53, 0.3],
            wall_alt: [0.6, 0.47, 0.26],
            brick: true,
            frame: [0.2, 0.18, 0.15],
            win: (0.2, 0.8, 0.18, 0.8),
            transom: true,
            ..base
        },
        _ => FacadeSpec {
            wall: [0.45, 0.36, 0.3],
            wall_alt: [0.5, 0.41, 0.34],
            frame: [0.12, 0.1, 0.09],
            win: (0.32, 0.68, 0.1, 0.85),
            piers: true,
            mullion: false,
            lit: 0.35,
            ..base
        },
    }
}

/// Returns (albedo, emissive) for a facade atlas of FAC_CELLS_X x FAC_CELLS_Y windows.
pub fn facade(kind: usize) -> (Image, Image) {
    let spec = facade_spec(kind);
    let seed = 100 + kind as u32 * 7;
    let (w, h) = (CW * FAC_CELLS_X, CH * FAC_CELLS_Y);
    let mut col = Canvas::new(w, h, spec.wall);
    let mut emi = Canvas::new(w, h, [0.0, 0.0, 0.0]);

    // wall
    col.fill_with(|x, y| {
        let n = fbm(x, y, w, h, 48, seed);
        let mut c = mix(spec.wall, spec.wall_alt, n);
        if spec.brick {
            let row = y / 5;
            let off = if row % 2 == 0 { 0 } else { 6 };
            let bx = (x + off) / 12;
            let mortar = (y % 5 == 4) || ((x + off) % 12 == 11);
            let v = 0.8 + 0.4 * hash(bx, row, seed + 3);
            c = mul(c, v);
            if mortar {
                c = mix(c, [0.62, 0.6, 0.55], 0.6);
            }
        } else {
            c = mul(c, 0.94 + 0.12 * hash(x, y, seed + 5));
            // panel seams / stone coursing
            if y % 26 == 0 {
                c = mul(c, 0.82);
            }
        }
        c
    });

    for cy in 0..FAC_CELLS_Y {
        for cx in 0..FAC_CELLS_X {
            let (ox, oy) = (cx * CW, cy * CH);
            let r = hash(cx, cy, seed + 11);
            let lit = r < spec.lit;
            let (fx0, fx1, fy0, fy1) = spec.win;
            let wx0 = ox + (fx0 * CW as f32) as i32;
            let wx1 = ox + (fx1 * CW as f32) as i32;
            let wy0 = oy + (fy0 * CH as f32) as i32;
            let wy1 = oy + (fy1 * CH as f32) as i32;
            let refl = 0.75 + 0.5 * hash(cx, cy, seed + 21);

            // spandrel (curtain wall band under the glass)
            if let Some(sp) = spec.spandrel {
                col.rect(ox, wy1, ox + CW, oy + CH, sp);
                col.rect(ox, wy1, ox + CW, wy1 + 2, mul(sp, 1.8));
            }
            // piers on art-deco style
            if spec.piers {
                col.blend(ox, oy, ox + 7, oy + CH, [0.62, 0.52, 0.42], 0.55);
                col.blend(ox + CW - 7, oy, ox + CW, oy + CH, [0.62, 0.52, 0.42], 0.55);
            }
            // sill and lintel
            if spec.sill {
                col.rect(wx0 - 2, wy1, wx1 + 2, wy1 + 3, mul(spec.wall, 1.25));
                col.rect(wx0 - 2, wy1 + 3, wx1 + 2, wy1 + 4, mul(spec.wall, 0.55));
                col.rect(wx0 - 1, wy0 - 3, wx1 + 1, wy0 - 1, mul(spec.wall, 1.15));
            }
            // frame
            col.rect(wx0, wy0, wx1, wy1, spec.frame);
            let fw = if spec.spandrel.is_some() { 1 } else { 2 };
            // glass with vertical gradient and reflection variation
            for y in (wy0 + fw)..(wy1 - fw) {
                let t = (y - wy0) as f32 / (wy1 - wy0) as f32;
                let g = mix(spec.glass_top, spec.glass_bot, t.powf(0.8));
                for x in (wx0 + fw)..(wx1 - fw) {
                    // faint diagonal sky streak
                    let streak = (((x - wx0) as f32 * 0.9 - (y - wy0) as f32 * 0.6) * 0.18).sin();
                    let s = 1.0 + 0.12 * streak * refl;
                    col.set(x, y, mul(g, refl * s));
                }
            }
            if spec.mullion {
                let mx = (wx0 + wx1) / 2;
                col.rect(mx - 1, wy0, mx + 1, wy1, spec.frame);
            }
            if spec.transom {
                let my = (wy0 + wy1) / 2;
                col.rect(wx0, my - 1, wx1, my + 1, spec.frame);
            }
            // balcony slab + railing
            if spec.balcony {
                col.rect(ox + 2, wy1 + 1, ox + CW - 2, wy1 + 5, mul(spec.wall, 1.12));
                col.rect(ox + 2, wy1 + 5, ox + CW - 2, wy1 + 6, mul(spec.wall, 0.5));
                let mut x = ox + 3;
                while x < ox + CW - 3 {
                    col.rect(x, wy1 + 6, x + 1, wy1 + 14, [0.18, 0.18, 0.2]);
                    x += 4;
                }
                col.rect(ox + 2, wy1 + 14, ox + CW - 2, wy1 + 16, [0.18, 0.18, 0.2]);
            }

            if lit {
                let warm = hash(cx, cy, seed + 31);
                let tint = if warm < 0.6 {
                    [1.0, 0.82, 0.5]
                } else if warm < 0.85 {
                    [1.0, 0.95, 0.8]
                } else {
                    [0.6, 0.75, 1.0]
                };
                let bright = 0.55 + 0.45 * hash(cx, cy, seed + 41);
                let curtain = hash(cx, cy, seed + 51) < 0.45;
                for y in (wy0 + fw)..(wy1 - fw) {
                    let t = (y - wy0) as f32 / (wy1 - wy0) as f32;
                    let k = if curtain && t < 0.3 { 0.25 } else { 1.0 };
                    for x in (wx0 + fw)..(wx1 - fw) {
                        let vig = 0.85 + 0.15 * hash(x, y, seed + 61);
                        emi.set(x, y, mul(tint, bright * k * vig));
                    }
                }
                if spec.mullion {
                    let mx = (wx0 + wx1) / 2;
                    emi.rect(mx - 1, wy0, mx + 1, wy1, [0.0, 0.0, 0.0]);
                }
                if spec.transom {
                    let my = (wy0 + wy1) / 2;
                    emi.rect(wx0, my - 1, wx1, my + 1, [0.0, 0.0, 0.0]);
                }
            }
        }
    }
    (col.into_image(), emi.into_image())
}

/// Ground-floor shop fronts: FAC_CELLS_X bays, one storey (4.2 m).
pub const SHOP_W: i32 = 48;
pub const SHOP_H: i32 = 68;

pub fn shopfront() -> (Image, Image) {
    let (w, h) = (SHOP_W * FAC_CELLS_X, SHOP_H);
    let mut col = Canvas::new(w, h, [0.4, 0.38, 0.36]);
    let mut emi = Canvas::new(w, h, [0.0, 0.0, 0.0]);
    col.fill_with(|x, y| mul([0.45, 0.42, 0.4], 0.9 + 0.2 * hash(x, y, 130)));
    let sign_cols: [Rgb; 6] = [
        [0.7, 0.12, 0.1],
        [0.1, 0.3, 0.6],
        [0.9, 0.75, 0.2],
        [0.15, 0.45, 0.25],
        [0.85, 0.85, 0.82],
        [0.25, 0.1, 0.35],
    ];
    for cx in 0..FAC_CELLS_X {
        let ox = cx * SHOP_W;
        let r = hash(cx, 0, 140);
        let sign = sign_cols[(hash(cx / 2, 7, 141) * 6.0) as usize % 6];
        // sign band
        col.rect(ox, 2, ox + SHOP_W, 14, mul(sign, 0.8));
        // fake lettering: random bright blocks
        let mut x = ox + 4;
        while x < ox + SHOP_W - 6 {
            let lw = 2 + (hash(x, 1, 142) * 4.0) as i32;
            let lit_c = if sign[0] + sign[1] + sign[2] > 1.9 {
                [0.1, 0.1, 0.1]
            } else {
                [0.95, 0.95, 0.9]
            };
            col.rect(x, 5, x + lw, 11, lit_c);
            emi.rect(x, 5, x + lw, 11, mul(lit_c, 0.9));
            x += lw + 2;
        }
        emi.rect(ox, 2, ox + SHOP_W, 4, mul(sign, 0.5));
        let is_door = r < 0.22;
        let shop_lit = hash(cx, 1, 143) < 0.88;
        let (gx0, gx1) = if is_door { (ox + 12, ox + 36) } else { (ox + 4, ox + 44) };
        let gy1 = if is_door { h - 1 } else { h - 12 };
        // frame + glass
        col.rect(gx0 - 2, 18, gx1 + 2, gy1 + 1, [0.12, 0.12, 0.13]);
        for y in 20..gy1 {
            let t = (y - 20) as f32 / (gy1 - 20) as f32;
            let g = mix([0.35, 0.45, 0.52], [0.1, 0.12, 0.14], t);
            for x in gx0..gx1 {
                col.set(x, y, g);
            }
        }
        if shop_lit {
            let tint = if hash(cx, 2, 144) < 0.5 {
                [1.0, 0.85, 0.6]
            } else {
                [0.85, 0.95, 1.0]
            };
            for y in 20..gy1 {
                for x in gx0..gx1 {
                    let shelf = if (y / 9) % 2 == 0 { 1.0 } else { 0.7 };
                    emi.set(x, y, mul(tint, 0.8 * shelf * (0.85 + 0.15 * hash(x, y, 145))));
                }
            }
        }
        if !is_door {
            let mx = (gx0 + gx1) / 2;
            col.rect(mx - 1, 18, mx + 1, gy1, [0.12, 0.12, 0.13]);
            emi.rect(mx - 1, 18, mx + 1, gy1, [0.0, 0.0, 0.0]);
            // bulkhead
            col.rect(ox, h - 12, ox + SHOP_W, h, [0.3, 0.28, 0.27]);
        } else {
            col.rect(ox + 22, 40, ox + 25, 44, [0.8, 0.75, 0.5]); // handle
        }
    }
    (col.into_image(), emi.into_image())
}

pub fn stripe_awning(c1: Rgb, c2: Rgb) -> Image {
    let (w, h) = (64, 8);
    let mut c = Canvas::new(w, h, c1);
    c.fill_with(|x, _| if (x / 8) % 2 == 0 { c1 } else { c2 });
    c.into_image()
}
