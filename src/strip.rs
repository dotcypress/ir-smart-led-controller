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
            animation_kind: AnimationKind::Static,
            color: IndexedColor::default(),
            on: false,
            frame: 0,
            overheat: false,
        }
    }

    pub fn animate(&self) -> Animation {
        if self.on {
            if self.overheat {
                Animation::new(
                    AnimationKind::Overheat,
                    0,
                    IndexedColor { luma: 1, code: 1 },
                )
            } else {
                Animation::new(self.animation_kind, self.frame, self.color)
            }
        } else {
            Animation::new(AnimationKind::Static, 0, IndexedColor::OFF)
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
            0 => self.luma_up(),
            1 => self.luma_down(),
            2 => self.switch_off(),
            3 => self.switch_on(),
            7 => self.color = self.color.with_code(0),
            11 => self.animation_kind = AnimationKind::Flash,
            15 => self.animation_kind = AnimationKind::Strobe,
            19 => self.animation_kind = AnimationKind::Fade,
            23 => self.animation_kind = AnimationKind::Smooth,
            _ => {
                let row = command / 4;
                let column = (command - row * 4) % 3;
                let color_code = column + (row - 1) * 3 + 1;
                self.color = self.color.with_code(color_code as usize);
            }
        }
    }

    fn switch_on(&mut self) {
        self.animation_kind = AnimationKind::Static;
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

    const PALETTE: [(u8, u8, u8); 16] = [
        (0xFF, 0xFF, 0xFF),
        (0xFF, 0x00, 0x00),
        (0x00, 0xFF, 0x00),
        (0x00, 0x00, 0xFF),
        (0xD3, 0x2F, 0x2F),
        (0x8B, 0xC3, 0x4A),
        (0x03, 0xA9, 0xF4),
        (0xFF, 0x98, 0x00),
        (0x4D, 0xD0, 0xE1),
        (0x8C, 0x17, 0xE0),
        (0xFF, 0x57, 0x22),
        (0x00, 0x96, 0x88),
        (0x9C, 0x27, 0xB0),
        (0xFF, 0xEB, 0x3B),
        (0x3F, 0x51, 0xB5),
        (0xE9, 0x1E, 0x63),
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
        if color.code > 15 {
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
}

impl Animation {
    pub fn new(kind: AnimationKind, frame: usize, color: IndexedColor) -> Self {
        Self {
            cursor: crate::STRIP_SIZE,
            kind,
            color,
            frame,
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
            AnimationKind::Overheat if self.frame % 10 < 5 => self.color,
            _ => IndexedColor::OFF,
        };

        self.cursor -= 1;
        Some(color.into())
    }
}
