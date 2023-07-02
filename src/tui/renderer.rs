use super::error::Result;
use super::canvas::Canvas;

pub(crate) trait Renderer {
    fn size_hint(&self) -> Result<(u16, u16)>;
    fn render(&mut self, c: &Canvas) -> Result<()>;
    fn clear(&mut self, c: &Canvas) -> Result<()>;
    fn recover(&mut self);
}
