use palette::rgb::Rgb as PaletteRgb;
use palette::stimulus::FromStimulus;
use palette::LightenAssign;

#[derive(Clone, Default)]
pub(crate) struct Rgb {
    color: PaletteRgb,
}

impl Rgb {
    pub(crate) fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            color: PaletteRgb::new(
                f32::from_stimulus(r),
                f32::from_stimulus(g),
                f32::from_stimulus(b),
            ),
        }
    }

    #[inline(always)]
    pub(crate) fn r(&self) -> u8 {
        u8::from_stimulus(self.color.red)
    }

    #[inline(always)]
    pub(crate) fn g(&self) -> u8 {
        u8::from_stimulus(self.color.green)
    }

    #[inline(always)]
    pub(crate) fn b(&self) -> u8 {
        u8::from_stimulus(self.color.blue)
    }

    pub(crate) fn set_lightness(&self, lightness: f32) -> Rgb {
        let lightness = if lightness > 1.0 {
            1.0
        } else {
            lightness
        };

        let mut new_color = self.clone();
        new_color.color.lighten_assign(lightness);
        new_color
    }
}

impl From<Rgb> for crossterm::style::Color {
    fn from(f: Rgb) -> crossterm::style::Color {
        crossterm::style::Color::Rgb {
            r: f.r(),
            g: f.g(),
            b: f.b(),
        }
    }
}
