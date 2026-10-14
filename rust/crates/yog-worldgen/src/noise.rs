//! Perlin noise, seeded: the same seed gives the same world on every
//! machine, in the game and in the IDE's preview.

/// Gradient noise in 2 and 3 dimensions, values in about -1..1.
#[derive(Debug, Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

/// A small, fast, well-mixed generator (SplitMix64) for seeds and placement.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// In `0..n`.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as u32 }
    }

    /// In `0.0..1.0`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `a`, `b` mixed into one seed (a chunk's, a layer's).
pub fn mix(a: u64, b: u64) -> u64 {
    Rng::new(a ^ b.rotate_left(32)).next_u64()
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn grad3(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 { y } else if h == 12 || h == 14 { x } else { z };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

impl Perlin {
    pub fn new(seed: u64) -> Perlin {
        let mut p: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut rng = Rng::new(seed);
        for i in (1..256).rev() {
            let j = rng.below(i as u32 + 1) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Perlin { perm }
    }

    pub fn get3(&self, x: f64, y: f64, z: f64) -> f64 {
        let (xi, yi, zi) = (x.floor() as i64 as usize & 255, y.floor() as i64 as usize & 255, z.floor() as i64 as usize & 255);
        let (xf, yf, zf) = (x - x.floor(), y - y.floor(), z - z.floor());
        let (u, v, w) = (fade(xf), fade(yf), fade(zf));
        let p = &self.perm;
        let a = p[xi] as usize + yi;
        let (aa, ab) = (p[a] as usize + zi, p[a + 1] as usize + zi);
        let b = p[xi + 1] as usize + yi;
        let (ba, bb) = (p[b] as usize + zi, p[b + 1] as usize + zi);
        lerp(
            w,
            lerp(
                v,
                lerp(u, grad3(p[aa], xf, yf, zf), grad3(p[ba], xf - 1.0, yf, zf)),
                lerp(u, grad3(p[ab], xf, yf - 1.0, zf), grad3(p[bb], xf - 1.0, yf - 1.0, zf)),
            ),
            lerp(
                v,
                lerp(u, grad3(p[aa + 1], xf, yf, zf - 1.0), grad3(p[ba + 1], xf - 1.0, yf, zf - 1.0)),
                lerp(u, grad3(p[ab + 1], xf, yf - 1.0, zf - 1.0), grad3(p[bb + 1], xf - 1.0, yf - 1.0, zf - 1.0)),
            ),
        )
    }

    pub fn get2(&self, x: f64, z: f64) -> f64 {
        self.get3(x, 0.5, z)
    }

    /// `octaves` layers, each twice as fine and `persistence` as strong.
    pub fn fbm2(&self, x: f64, z: f64, octaves: u32, persistence: f64) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves.max(1) {
            sum += self.get2(x * freq, z * freq) * amp;
            norm += amp;
            amp *= persistence;
            freq *= 2.0;
        }
        sum / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_noise_and_values_stay_in_range() {
        let (a, b) = (Perlin::new(42), Perlin::new(42));
        let c = Perlin::new(43);
        let mut differ = false;
        for i in 0..200 {
            let (x, z) = (i as f64 * 0.37, i as f64 * 0.71);
            assert_eq!(a.get2(x, z), b.get2(x, z));
            differ |= a.get2(x, z) != c.get2(x, z);
            let v = a.fbm2(x, z, 4, 0.5);
            assert!((-1.1..=1.1).contains(&v), "{v}");
        }
        assert!(differ, "another seed, another world");
        assert_eq!(a.get3(3.0, 4.0, 5.0), 0.0, "zero on the lattice");
    }
}
