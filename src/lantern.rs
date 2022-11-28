const PALLETE: [(u8, u8, u8); 8] = [
    (3, 1, 0),
    (7, 3, 0),
    (12, 5, 0),
    (20, 8, 0),
    (28, 10, 0),
    (30, 12, 0),
    (32, 14, 0),
    (40, 16, 0),
];

pub struct Fire {
    seed: usize,
    animation: [u8; 4],
}

impl Fire {
    pub fn new() -> Self {
        Self {
            seed: 42,
            animation: [0; 4],
        }
    }

    pub fn animate(&mut self) -> [(u8, u8, u8); 4] {
        for el in self.animation.iter_mut() {
            self.seed = self.seed * 16_807 % 0x7fff_ffff;
            let rnd = self.seed as u8;
            if rnd < 100 {
                *el = el.saturating_sub(1);
            } else if rnd > 150 {
                *el = el.saturating_add(1).min(PALLETE.len() as u8 - 1);
            }
        }
        self.animation.map(|c| PALLETE[c as usize])
    }
}

pub struct Lantern {
    on: bool,
    fire: Fire,
}

impl Lantern {
    pub fn new() -> Self {
        Self {
            on: true,
            fire: Fire::new(),
        }
    }

    pub fn command(&mut self, cmd: u8) {
        match cmd {
            0 => self.on = true,
            1 => self.on = false,
            _ => {}
        }
    }

    pub fn animate(&mut self) -> [(u8, u8, u8); 4] {
        if self.on {
            self.fire.animate()
        } else {
            [(0, 0, 0); 4]
        }
    }
}
