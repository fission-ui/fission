use crate::state::AnimationGalleryState;
use crate::style::*;
use fission::build::BuildCtxHandle;
use fission::{Button, ButtonVariant, Column, Container, Row, Text, Widget};
use serde::Serialize;

pub struct PageHeader<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
}

impl From<PageHeader<'_>> for Widget {
    fn from(header: PageHeader<'_>) -> Self {
        Column {
            gap: Some(5.0),
            children: vec![
                Text::new(header.title).size(22.0).color(INK).into(),
                Text::new(header.subtitle).size(12.0).color(MUTED).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct PageNote<'a> {
    pub title: &'a str,
    pub body: &'a str,
}

impl From<PageNote<'_>> for Widget {
    fn from(note: PageNote<'_>) -> Self {
        Container::new(Column {
            gap: Some(4.0),
            children: vec![
                Text::new(note.title).size(12.0).color(BLUE).into(),
                Text::new(note.body).size(11.0).color(MUTED).into(),
            ],
            ..Default::default()
        })
        .padding_all(12.0)
        .border_radius(12.0)
        .bg(color(242, 245, 255, 255))
        .into()
    }
}

pub struct SectionTitle<'a> {
    pub title: &'a str,
}

impl From<SectionTitle<'_>> for Widget {
    fn from(title: SectionTitle<'_>) -> Self {
        Text::new(title.title).size(13.0).color(INK).into()
    }
}

pub struct LabelRow<'a> {
    pub label: &'a str,
}

impl From<LabelRow<'_>> for Widget {
    fn from(row: LabelRow<'_>) -> Self {
        Row {
            gap: Some(7.0),
            children: vec![
                Container::new(Text::new(" "))
                    .width(8.0)
                    .height(8.0)
                    .border_radius(4.0)
                    .bg(TEAL)
                    .into(),
                Text::new(row.label).size(11.0).color(MUTED).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct LabelValue<'a> {
    pub label: &'a str,
    pub value: &'a str,
}

impl From<LabelValue<'_>> for Widget {
    fn from(value: LabelValue<'_>) -> Self {
        Row {
            gap: Some(8.0),
            children: vec![
                Container::new(Text::new(value.label).size(11.0).color(MUTED))
                    .width(104.0)
                    .into(),
                Text::new(value.value).size(11.0).color(INK).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub struct CodeBlock<'a> {
    pub source: &'a str,
}

impl From<CodeBlock<'_>> for Widget {
    fn from(block: CodeBlock<'_>) -> Self {
        Container::new(
            Text::new(block.source)
                .size(11.0)
                .color(color(35, 44, 70, 255)),
        )
        .padding_all(12.0)
        .border_radius(12.0)
        .border(color(226, 230, 239, 255), 1.0)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}

pub struct SmallButton<'a, A>
where
    A: fission::Action + Serialize + Clone + 'static,
{
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub label: &'a str,
    pub action: A,
    pub reducer: fission::Handler<AnimationGalleryState, A>,
}

impl<A> From<SmallButton<'_, A>> for Widget
where
    A: fission::Action + Serialize + Clone + 'static,
{
    fn from(button: SmallButton<'_, A>) -> Self {
        Button {
            variant: ButtonVariant::Outline,
            child: Some(Text::new(button.label).size(11.0).into()),
            on_press: Some(button.ctx.bind(button.action, button.reducer)),
            ..Default::default()
        }
        .into()
    }
}

pub struct ChoiceButton<'a, A>
where
    A: fission::Action + Serialize + Clone + 'static,
{
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub label: &'a str,
    pub active: bool,
    pub action: A,
    pub reducer: fission::Handler<AnimationGalleryState, A>,
}

impl<A> From<ChoiceButton<'_, A>> for Widget
where
    A: fission::Action + Serialize + Clone + 'static,
{
    fn from(button: ChoiceButton<'_, A>) -> Self {
        Button {
            variant: if button.active {
                ButtonVariant::Filled
            } else {
                ButtonVariant::Outline
            },
            child: Some(Text::new(button.label).size(11.0).into()),
            on_press: Some(button.ctx.bind(button.action, button.reducer)),
            ..Default::default()
        }
        .into()
    }
}

pub struct ColorDots<'a> {
    pub colors: &'a [fission::op::Color],
}

impl From<ColorDots<'_>> for Widget {
    fn from(dots: ColorDots<'_>) -> Self {
        Row {
            gap: Some(4.0),
            children: dots
                .colors
                .iter()
                .map(|c| {
                    Container::new(Text::new(" "))
                        .width(9.0)
                        .height(9.0)
                        .border_radius(5.0)
                        .bg(*c)
                        .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}

pub struct Bullet<'a> {
    pub text: &'a str,
}

impl From<Bullet<'_>> for Widget {
    fn from(bullet: Bullet<'_>) -> Self {
        Row {
            gap: Some(7.0),
            children: vec![
                Text::new("+").size(11.0).color(BLUE).into(),
                Text::new(bullet.text).size(11.0).color(MUTED).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}
