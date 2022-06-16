use vizia::prelude::*;

use crate::ui_layer::{AppData, AppEvent, ChannelData, PanelEvent, PanelState, PatternData};

#[derive(Debug, Clone, Copy, PartialEq, Data)]
pub enum ChannelRackOrientation {
    Horizontal,
    Vertical,
}

impl Default for ChannelRackOrientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

impl From<ChannelRackOrientation> for bool {
    fn from(orientation: ChannelRackOrientation) -> bool {
        match orientation {
            ChannelRackOrientation::Vertical => true,
            ChannelRackOrientation::Horizontal => false,
        }
    }
}

pub fn channels(cx: &mut Context) {
    VStack::new(cx, |cx| {
        // Although this is a vstack we're using css to switch between horizontal and vertical layouts
        VStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                // TODO - Make this resizable when channel rack orientation is vertical
                VStack::new(cx, |cx| {
                    // Header
                    HStack::new(cx, |cx| {
                        Label::new(cx, "CHANNEL RACK").class("small");

                        Button::new(
                            cx,
                            |cx| {
                                cx.emit(PanelEvent::ToggleChannelRackOrientation);
                                cx.emit(PanelEvent::ShowPatterns);
                            },
                            |cx| Label::new(cx, "A"),
                        )
                        .child_space(Stretch(1.0))
                        .width(Pixels(24.0))
                        .left(Stretch(1.0));

                        Button::new(
                            cx,
                            |cx| cx.emit(PanelEvent::TogglePatterns),
                            |cx| Label::new(cx, "B"),
                        )
                        .child_space(Stretch(1.0))
                        .width(Pixels(24.0))
                        .right(Pixels(10.0));
                    })
                    .class("header");

                    // Contents
                    ScrollView::new(cx, 0.0, 0.0, false, false, |cx| {
                        // Master Channel
                        HStack::new(cx, |cx| {
                            Element::new(cx)
                                .width(Pixels(14.0))
                                .background_color(Color::from("#D4D5D5"))
                                .class("bar");

                            VStack::new(cx, |cx| {
                                Label::new(
                                    cx,
                                    AppData::channel_data.index(0).then(ChannelData::name),
                                );
                            });
                        })
                        .class("channel")
                        .toggle_class(
                            "selected",
                            AppData::channel_data.index(0).then(ChannelData::selected),
                        )
                        .on_press(move |cx| cx.emit(AppEvent::SelectChannel(0)));

                        // Other Channels
                        List::new(
                            cx,
                            AppData::channel_data.index(0).then(ChannelData::subchannels),
                            |cx, _, item| {
                                channel(cx, AppData::channel_data, item.get(cx), 0);
                            },
                        )
                        .row_between(Pixels(4.0));
                    })
                    //.child_space(Pixels(4.0))
                    //.row_between(Pixels(4.0))
                    .class("channels_content")
                    .class("level3");
                })
                .row_between(Pixels(1.0))
                .width(Pixels(225.0))
                .class("instruments");

                VStack::new(cx, |cx| {
                    HStack::new(cx, |cx| {
                        Label::new(cx, "PATTERNS").class("small");
                    })
                    .class("header");

                    // Contents
                    VStack::new(cx, |cx| {}).class("level3");
                })
                .row_between(Pixels(1.0))
                .class("patterns")
                .checked(PanelState::hide_patterns);
            })
            .overflow(Overflow::Hidden);

            VStack::new(cx, |cx| {
                // TODO - De-duplicate this code
                HStack::new(cx, |cx| {
                    Label::new(cx, "PATTERNS").class("small").text_wrap(false);
                })
                .class("header");

                // Contents
                ScrollView::new(cx, 0.0, 0.0, false, false, |cx| {
                    List::new(cx, AppData::pattern_data, |cx, _, pattern| {
                        let channel_index = pattern.get(cx).channel;
                        VStack::new(cx, |cx| {
                            Label::new(cx, pattern.then(PatternData::name))
                                .text_wrap(false)
                                .background_color(
                                    AppData::channel_data
                                        .index(channel_index)
                                        .then(ChannelData::color),
                                );
                        })
                        .visibility(
                            AppData::channel_data.index(channel_index).then(ChannelData::selected),
                        )
                        .class("pattern");
                    })
                    .child_space(Pixels(4.0));
                })
                .class("level3");
            })
            .row_between(Pixels(1.0))
            .class("patterns")
            .checked(PanelState::hide_patterns);
        })
        .toggle_class("vertical", PanelState::channel_rack_orientation.map(|&val| val.into()))
        .toggle_class("hidden", PanelState::hide_patterns)
        .class("channels");
    })
    .class("channel_rack");
}

// This doesn't work because of some mess with recursion, closures, and generics :(
// https://stackoverflow.com/questions/54613966/error-reached-the-recursion-limit-while-instantiating-funcclosure
// pub fn channel<L: Lens<Target = ChannelData>>(cx: &mut Context, item: L)
// where <L as Lens>::Source: Model,
// {
//     VStack::new(cx, |cx|{
//         Element::new(cx)
//             .height(Pixels(50.0))
//             .border_width(Pixels(1.0))
//             .border_color(Color::red());
//         if !item.get(cx).subchannels.is_empty() {
//             List::new(cx, item.then(ChannelData::subchannels), |cx, idx, subitem|{
//                 channel(cx, subitem);
//             });
//         }
//     });
// }

pub fn channel<L: Lens<Target = Vec<ChannelData>>>(
    cx: &mut Context,
    root: L,
    index: usize,
    level: usize,
) where
    <L as Lens>::Source: Model,
{
    VStack::new(cx, |cx| {
        let new_root = root.clone();
        Binding::new(cx, root.index(index), move |cx, chnl| {
            let data = chnl.get(cx);

            HStack::new(cx, |cx| {
                let is_grouped = !data.subchannels.is_empty();
                Element::new(cx)
                    .width(Pixels(14.0))
                    .background_color(data.color.clone())
                    .class("bar")
                    .toggle_class("grouped", is_grouped);

                VStack::new(cx, |cx| {
                    Label::new(cx, chnl.then(ChannelData::name));
                });
            })
            .class("channel")
            .toggle_class("selected", data.selected)
            .on_press(move |cx| cx.emit(AppEvent::SelectChannel(index)));

            HStack::new(cx, |cx| {
                //Spacer
                Element::new(cx)
                    //.background_color(data.color.clone())
                    .class("group_bar");
                VStack::new(cx, |cx| {
                    for idx in data.subchannels.iter() {
                        let new_root = new_root.clone();
                        channel(cx, new_root, *idx, level + 1);
                    }
                })
                .class("channel_group");
            })
            .border_radius_bottom_left(Pixels(2.0))
            .background_color(data.color);
        });
    })
    //.left(Pixels(10.0 * level as f32))
    .height(Auto);
}