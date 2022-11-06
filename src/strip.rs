use smart_leds::RGB8;

pub struct Strip {
    animation_kind: AnimationKind,
    color: IndexedColor,
    frame: usize,
    on: bool,
    overheat: bool,
}

impl Default for Strip {
    fn default() -> Self {
        Self::new()
    }
}

impl Strip {
    pub fn new() -> Self {
        Self {
            animation_kind: AnimationKind::Fire,
            color: IndexedColor::default(),
            on: true,
            frame: 0,
            overheat: false,
        }
    }

    pub fn on(&self) -> bool {
        self.on
    }

    pub fn animate(&self) -> Animation {
        match (self.on, self.overheat) {
            (true, false) => Animation::new(self.animation_kind, self.frame, self.color),
            (true, true) => Animation::new(AnimationKind::Overheat, self.frame, IndexedColor::RED),
            _ => Animation::new(AnimationKind::Static, 0, IndexedColor::OFF),
        }
    }

    pub fn handle_overheat(&mut self) {
        self.overheat = true;
    }

    pub fn handle_frame(&mut self) {
        self.frame += 1;
    }

    pub fn handle_command(&mut self, command: u8) {
        match command {
            0 => self.switch_off(),
            13 => self.switch_off(),
            1 => self.switch_on(),
            2 => self.animation_kind = AnimationKind::Static,
            22 => self.switch_on(),
            24 => self.luma_up(),
            82 => self.luma_down(),
            69 => self.color = self.color.with_code(7),
            70 => self.color = self.color.with_code(10),
            71 => self.color = self.color.with_code(13),
            68 => self.color = self.color.with_code(14),
            64 => self.color = self.color.with_code(15),
            67 => self.color = self.color.with_code(16),
            11 => self.animation_kind = AnimationKind::Flash,
            15 => self.animation_kind = AnimationKind::Strobe,
            19 => self.animation_kind = AnimationKind::Fade,
            23 => self.animation_kind = AnimationKind::Smooth,
            x => {
                self.color = self.color.with_code(x as usize % 16);
            }
        }
    }

    fn switch_on(&mut self) {
        self.animation_kind = AnimationKind::Fire;
        self.color = IndexedColor::default();
        self.on = true;
        self.overheat = false;
    }

    fn switch_off(&mut self) {
        self.on = false;
    }

    fn luma_up(&mut self) {
        let luma = self.color.luma();
        if luma < 5 {
            self.color = self.color.with_luma(luma + 1);
        }
    }

    fn luma_down(&mut self) {
        let luma = self.color.luma();
        if luma > 0 {
            self.color = self.color.with_luma(luma - 1);
        }
    }
}

#[derive(Clone, Copy)]
pub enum AnimationKind {
    Static,
    Flash,
    Strobe,
    Fade,
    Smooth,
    Fire,
    Overheat,
}

#[derive(Clone, Copy, Default)]
pub struct IndexedColor {
    code: usize,
    luma: usize,
}

impl IndexedColor {
    pub const OFF: Self = Self {
        code: usize::MAX,
        luma: 0,
    };

    pub const RED: Self = Self { code: 1, luma: 0 };

    const PALETTE: [(u8, u8, u8); 26] = [
        (0xff, 0xff, 0xff),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0x00, 0x00, 0xff),
        (0xd3, 0x2f, 0x2f),
        (0x8b, 0xc3, 0x4a),
        (0x03, 0xa9, 0xf4),
        (0xff, 0x98, 0x00),
        (0x4d, 0xd0, 0xe1),
        (0x8c, 0x17, 0xe0),
        (0xff, 0x57, 0x22),
        (0x00, 0x96, 0x88),
        (0x9c, 0x27, 0xb0),
        (0xff, 0xeb, 0x3b),
        (0x3f, 0x51, 0xb5),
        (0xe9, 0x1e, 0x63),
        (0x03, 0x07, 0x1e),
        (0x37, 0x0f, 0x00),
        (0x6a, 0x0f, 0x00),
        (0x9d, 0x0f, 0x00),
        (0xd0, 0x0f, 0x00),
        (0x6d, 0x2f, 0x00),
        (0x7d, 0x3f, 0x10),
        (0x94, 0x4f, 0x00),
        (0xaa, 0x2f, 0x00),
        (0x00, 0x00, 0x00),
    ];

    pub fn code(&self) -> usize {
        self.code
    }

    pub fn luma(&self) -> usize {
        self.luma
    }

    pub fn with_code(&self, code: usize) -> Self {
        Self {
            code,
            luma: self.luma,
        }
    }

    pub fn with_luma(&self, luma: usize) -> Self {
        assert!(self.luma <= 5);
        Self {
            luma,
            code: self.code,
        }
    }
}

impl From<IndexedColor> for RGB8 {
    fn from(color: IndexedColor) -> Self {
        if color.code > 25 {
            return RGB8::default();
        }
        let (r, g, b) = IndexedColor::PALETTE[color.code];
        assert!(color.luma <= 5);
        let shift = 5 - color.luma;
        RGB8::new(r >> shift, g >> shift, b >> shift)
    }
}

pub struct Animation {
    kind: AnimationKind,
    color: IndexedColor,
    cursor: usize,
    frame: usize,
    seed: usize,
}

impl Animation {
    pub fn new(kind: AnimationKind, frame: usize, color: IndexedColor) -> Self {
        Self {
            cursor: crate::STRIP_SIZE,
            kind,
            color,
            frame,
            seed: frame,
        }
    }
}

impl Iterator for Animation {
    type Item = RGB8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor == 0 {
            return None;
        }
        let color = match self.kind {
            AnimationKind::Static => self.color,
            AnimationKind::Flash if self.frame % 10 < 5 => self.color,
            AnimationKind::Strobe if self.frame % 15 < 9 && self.frame % 2 == 0 => self.color,
            AnimationKind::Smooth => {
                let color = (self.frame >> 2) % 5;
                self.color.with_code(color * 3 + 1)
            }
            AnimationKind::Fade => {
                let mut luma = self.color.luma();
                if luma < 1 {
                    luma = 1;
                }
                let luma = luma.saturating_sub(5 - (self.frame >> 1) % 6);
                self.color.with_luma(luma)
            }
            AnimationKind::Fire => {
                self.seed = self.seed * 16_807 % 0x7fff_ffff;
                let rnd = ((self.frame + self.seed) as u8) as usize;
                self.color.with_code(21 + rnd % 4)
            }
            AnimationKind::Overheat if self.frame % 10 < 5 => {
                let color = self.color;
                self.color = IndexedColor::OFF;
                color
            }
            _ => IndexedColor::OFF,
        };

        self.cursor -= 1;
        Some(color.into())
    }
}
