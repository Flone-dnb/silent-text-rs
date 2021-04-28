use iced::{
    button, executor, text_input, window::icon::Icon, Align, Application, Button, Clipboard, Color,
    Column, Command, Element, HorizontalAlignment, Length, Row, Settings, Text, TextInput,
    VerticalAlignment,
};

use std::fs::File;

mod global_params;
mod themes;
mod widgets;
use global_params::*;
use themes::StyleTheme;
use themes::Theme;
use widgets::chat_list::ChatList;
use widgets::users_list::UsersList;

fn main() -> iced::Result {
    let mut config = Settings::default();
    config.antialiasing = false;
    config.window.size = (900, 500);
    config.window.min_size = Some((900, 500));
    config.default_font = Some(include_bytes!("../res/mplus-2p-light.ttf"));

    let icon = Icon::from_rgba(read_icon_png(String::from("res/app_icon.png")), 256, 256).unwrap();
    config.window.icon = Some(icon);
    config.default_text_size = 28;
    Silent::run(config)
}

fn read_icon_png(path: String) -> Vec<u8> {
    let decoder = png::Decoder::new(File::open(path).unwrap());
    let (info, mut reader) = decoder.read_info().unwrap();
    let mut buf = vec![0; info.buffer_size()];

    reader.next_frame(&mut buf).unwrap();

    buf
}

#[derive(Debug, Clone)]
enum WindowLayout {
    ConnectWindow,
    MainWindow,
}

#[derive(Debug)]
struct Silent {
    connect_window: ConnectWindow,

    chat_list: ChatList,
    users_list: UsersList,

    message_string: String,

    current_window_layout: WindowLayout,

    style: StyleTheme,

    message_input: text_input::State,
}

#[derive(Debug, Default)]
struct ConnectWindow {
    username_string: String,
    servername_string: String,
    port_string: String,
    password_string: String,

    username_input: text_input::State,
    servername_input: text_input::State,
    port_input: text_input::State,
    password_input: text_input::State,
    connect_button: button::State,
    settings_button: button::State,
}

#[derive(Debug, Clone)]
enum MainMessage {
    MessageInputChanged(String),
    UsernameInputChanged(String),
    ServernameInputChanged(String),
    PortInputChanged(String),
    PasswordInputChanged(String),
    ConnectButtonPressed,
    SettingsButtonPressed,
}

impl Silent {
    fn new() -> Self {
        Silent {
            chat_list: ChatList::new(),
            users_list: UsersList::default(),
            current_window_layout: WindowLayout::ConnectWindow,
            style: StyleTheme::new(Theme::Default),
            message_string: String::default(),
            message_input: text_input::State::default(),
            connect_window: ConnectWindow::default(),
        }
    }
}

impl Application for Silent {
    type Executor = executor::Default;
    type Message = MainMessage;
    type Flags = ();

    fn new(_flags: ()) -> (Silent, Command<MainMessage>) {
        (Silent::new(), Command::none())
    }

    fn background_color(&self) -> Color {
        self.style.get_background_color()
    }

    fn title(&self) -> String {
        String::from("Silent")
    }

    fn update(
        &mut self,
        message: Self::Message,
        clipboard: &mut Clipboard,
    ) -> Command<Self::Message> {
        match message {
            MainMessage::MessageInputChanged(text) => {
                if text.chars().count() <= MAX_MESSAGE_SIZE {
                    self.message_string = text
                }
            }
            MainMessage::UsernameInputChanged(text) => {
                if text.chars().count() <= MAX_USERNAME_SIZE {
                    self.connect_window.username_string = text;
                }
            }
            MainMessage::ServernameInputChanged(text) => {
                self.connect_window.servername_string = text;
            }
            MainMessage::PortInputChanged(text) => {
                if text.chars().count() <= 5 {
                    self.connect_window.port_string = text;
                }
            }
            MainMessage::PasswordInputChanged(text) => {
                self.connect_window.password_string = text;
            }
            MainMessage::ConnectButtonPressed => {}
            MainMessage::SettingsButtonPressed => {}
        }

        Command::none()
    }

    fn view(&mut self) -> Element<MainMessage> {
        match self.current_window_layout {
            WindowLayout::ConnectWindow => Column::new()
                .align_items(Align::Center)
                .push(Column::new().height(Length::FillPortion(10)))
                .push(
                    Row::new()
                        .spacing(5)
                        .height(Length::FillPortion(30))
                        .push(Column::new().width(Length::FillPortion(30)))
                        .push(
                            Column::new()
                                .width(Length::FillPortion(15))
                                .spacing(10)
                                .padding(5)
                                .push(Text::new("Username: ").color(Color::WHITE))
                                .push(Text::new("Server: ").color(Color::WHITE))
                                .push(Text::new("Port: ").color(Color::WHITE))
                                .push(Text::new("Password: ").color(Color::WHITE)),
                        )
                        .push(
                            Column::new()
                                .width(Length::FillPortion(25))
                                .spacing(10)
                                .padding(5)
                                .push(
                                    TextInput::new(
                                        &mut self.connect_window.username_input,
                                        "Type your username...",
                                        &self.connect_window.username_string,
                                        MainMessage::UsernameInputChanged,
                                    )
                                    .style(self.style.theme),
                                )
                                .push(
                                    TextInput::new(
                                        &mut self.connect_window.servername_input,
                                        "IP or domain name...",
                                        &self.connect_window.servername_string,
                                        MainMessage::ServernameInputChanged,
                                    )
                                    .style(self.style.theme),
                                )
                                .push(
                                    TextInput::new(
                                        &mut self.connect_window.port_input,
                                        "",
                                        &self.connect_window.port_string,
                                        MainMessage::PortInputChanged,
                                    )
                                    .style(self.style.theme),
                                )
                                .push(
                                    TextInput::new(
                                        &mut self.connect_window.password_input,
                                        "(optional)",
                                        &self.connect_window.password_string,
                                        MainMessage::PasswordInputChanged,
                                    )
                                    .style(self.style.theme),
                                ),
                        )
                        .push(Column::new().width(Length::FillPortion(30))),
                )
                .push(Column::new().height(Length::FillPortion(5)))
                .push(
                    Row::new()
                        .height(Length::Shrink)
                        .push(Column::new().width(Length::FillPortion(40)))
                        .push(
                            Button::new(
                                &mut self.connect_window.connect_button,
                                Text::new("Connect").color(Color::WHITE),
                            )
                            .on_press(MainMessage::ConnectButtonPressed)
                            .width(Length::FillPortion(20))
                            .height(Length::Shrink)
                            .style(self.style.theme),
                        )
                        .push(Column::new().width(Length::FillPortion(40))),
                )
                .push(Column::new().height(Length::FillPortion(30)))
                .push(
                    Row::new()
                        .height(Length::Shrink)
                        .push(Column::new().width(Length::FillPortion(40)))
                        .push(
                            Button::new(
                                &mut self.connect_window.settings_button,
                                Text::new("Settings").color(Color::WHITE),
                            )
                            .on_press(MainMessage::SettingsButtonPressed)
                            .width(Length::FillPortion(20))
                            .height(Length::Shrink)
                            .style(self.style.theme),
                        )
                        .push(Column::new().width(Length::FillPortion(40))),
                )
                .push(Column::new().height(Length::FillPortion(10)))
                .into(),
            WindowLayout::MainWindow => {
                self.chat_list.add_message(
                    String::from("Привет мир! Hello World!"),
                    String::from("Bar"),
                );

                self.chat_list.add_message(
                    String::from("Привет мир! Hello World!"),
                    String::from("Foo"),
                );

                self.chat_list
                    .add_message(String::from("Addition string!"), String::from("Foo"));

                self.users_list.add_user(String::from("Bar"));
                self.users_list.add_user(String::from("Foo"));

                let left: Column<MainMessage> = Column::new()
                    .align_items(Align::Center)
                    .padding(5)
                    .spacing(10)
                    .push(
                        Text::new("Text Chat")
                            .horizontal_alignment(HorizontalAlignment::Center)
                            .vertical_alignment(VerticalAlignment::Center)
                            .color(Color::WHITE)
                            .height(Length::FillPortion(8)),
                    )
                    .push(
                        self.chat_list
                            .get_ui(&self.style)
                            .height(Length::FillPortion(85)),
                    )
                    .push(
                        Row::new()
                            .push(
                                TextInput::new(
                                    &mut self.message_input,
                                    "Type your message here...",
                                    &self.message_string,
                                    MainMessage::MessageInputChanged,
                                )
                                .size(22)
                                .style(self.style.theme),
                            )
                            .height(Length::FillPortion(7)),
                    );

                let right: Column<MainMessage> = Column::new()
                    .align_items(Align::Center)
                    .padding(5)
                    .spacing(10)
                    .push(
                        Text::new("Connected: 0")
                            .horizontal_alignment(HorizontalAlignment::Center)
                            .vertical_alignment(VerticalAlignment::Center)
                            .color(Color::WHITE)
                            .height(Length::FillPortion(8)),
                    )
                    .push(
                        self.users_list
                            .get_ui(&self.style)
                            .width(Length::Fill)
                            .height(Length::FillPortion(92)),
                    );

                Row::new()
                    .padding(10)
                    .spacing(0)
                    .align_items(Align::Center)
                    .push(left.width(Length::FillPortion(65)))
                    .push(right.width(Length::FillPortion(35)))
                    .into()
            }
        }
    }
}
