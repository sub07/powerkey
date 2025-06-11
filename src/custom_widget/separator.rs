use iced::{
    Border, Element, Length, Shadow, Size,
    advanced::{self, Widget, layout::Node},
};

pub struct Separator {
    stroke_width: f32,
}

impl Separator {
    pub fn stroke_width(mut self, stroke_width: f32) -> Self {
        self.stroke_width = stroke_width;
        self
    }
}

pub fn separator() -> Separator {
    Separator { stroke_width: 1.0 }
}

impl<M, T, R> From<Separator> for Element<'_, M, T, R>
where
    R: advanced::renderer::Renderer,
{
    fn from(circle: Separator) -> Self {
        Self::new(circle)
    }
}

impl<M, T, R> Widget<M, T, R> for Separator
where
    R: advanced::renderer::Renderer,
{
    fn size(&self) -> iced::Size<iced::Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &self,
        _tree: &mut advanced::widget::Tree,
        _renderer: &R,
        limits: &advanced::layout::Limits,
    ) -> advanced::layout::Node {
        Node::new(Size::new(limits.max().width, self.stroke_width))
    }

    fn draw(
        &self,
        _tree: &advanced::widget::Tree,
        renderer: &mut R,
        _theme: &T,
        style: &advanced::renderer::Style,
        layout: advanced::Layout<'_>,
        _cursor: advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        renderer.fill_quad(
            advanced::renderer::Quad {
                bounds: layout.bounds(),
                border: Border::default(),
                shadow: Shadow::default(),
            },
            style.text_color.scale_alpha(0.2),
        );
    }
}
