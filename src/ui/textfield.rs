//! A fairly simplistic text field implementation.

use std::{borrow::BorrowMut, ops::Range};

use copypasta::{ClipboardContext, ClipboardProvider};
use skulpin::skia_safe::*;

use crate::ui::*;

/// Text field selection struct
struct Selection {
    pub first: usize,
    pub second: usize,
}

impl Selection {
    pub fn start(&self) -> usize {
        self.first.min(self.second)
    }

    pub fn end(&self) -> usize {
        self.first.max(self.second)
    }

    pub fn normalize(&self) -> Range<usize> {
        self.start()..self.end()
    }

    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    pub fn move_cursors(&mut self, position: usize) {
        self.first = position;
        self.second = self.first;
    }

    pub fn move_cursors_left(&mut self) {
        if self.first > 0 {
            self.first -= 1;
            self.second = self.first;
        }
    }

    pub fn move_cursors_right(&mut self) {
        self.first += 1;
        self.second = self.first;
    }
}

/// A text field's state.
pub struct TextField {
    text: Vec<char>,
    text_utf8: String,
    focused: bool,
    blink_start: f32,

    selection: Selection,

    clipboard_context: ClipboardContext,
}

/// A text field's color scheme.
#[derive(Clone)]
pub struct TextFieldColors {
    pub outline: Color,
    pub outline_focus: Color,
    pub fill: Color,
    pub text: Color,
    pub text_hint: Color,
    pub label: Color,
    pub selection: Color,
}

/// Processing arguments for a text field.
#[derive(Clone, Copy)]
pub struct TextFieldArgs<'a, 'b> {
    pub width: f32,
    pub colors: &'a TextFieldColors,
    pub hint: Option<&'b str>,
}

impl TextField {
    /// The backspace character.
    const BACKSPACE: char = '\x08';
    /// The blinking period of the caret.
    const BLINK_PERIOD: f32 = 1.0;
    const HALF_BLINK: f32 = Self::BLINK_PERIOD / 2.0;

    /// Creates a new text field, with the optionally provided initial text.
    pub fn new(initial_text: Option<&str>) -> Self {
        let text_utf8: String = initial_text.unwrap_or("").into();
        let length = text_utf8.len();

        Self {
            text: text_utf8.chars().collect(),
            text_utf8,
            focused: false,
            blink_start: 0.0,

            selection: Selection {
                first: length,
                second: length,
            },

            clipboard_context: ClipboardContext::new().unwrap(),
        }
    }

    /// Updates the text field's UTF-8 string.
    fn update_utf8(&mut self) {
        self.text_utf8 = self.text.iter().collect();
    }

    /// Returns the height of a text field.
    pub fn height(ui: &Ui) -> f32 {
        f32::round(16.0 / 7.0 * ui.font_size())
    }

    /// Processes a text field.
    pub fn process(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        TextFieldArgs { width, colors, hint }: TextFieldArgs,
    ) {
        ui.push_group((width, Self::height(ui)), Layout::Freeform);

        // Rendering: box
        ui.draw_on_canvas(canvas, |canvas| {
            let mut paint = Paint::new(Color4f::from(colors.fill), None);
            paint.set_anti_alias(true);
            let mut rrect = RRect::new_rect_xy(&Rect::from_point_and_size((0.0, 0.0), ui.size()), 4.0, 4.0);
            canvas.draw_rrect(rrect, &paint);
            paint.set_color(if self.focused {
                colors.outline_focus
            } else {
                colors.outline
            });
            paint.set_style(paint::Style::Stroke);
            rrect.offset((0.5, 0.5));
            canvas.draw_rrect(rrect, &paint);
        });

        // Rendering: text
        ui.push_group(ui.size(), Layout::Freeform);
        ui.pad((16.0, 0.0));
        canvas.save();
        ui.clip(canvas);

        // Rendering: hint
        if hint.is_some() && self.text.len() == 0 {
            ui.text(canvas, hint.unwrap(), colors.text_hint, (AlignH::Left, AlignV::Middle));
        }

        ui.text(canvas, &self.text_utf8, colors.text, (AlignH::Left, AlignV::Middle));

        if self.focused && (input.time_in_seconds() - self.blink_start) % Self::BLINK_PERIOD < Self::HALF_BLINK {
            ui.draw_on_canvas(canvas, |canvas| {
                let mut paint = Paint::new(Color4f::from(colors.text), None);
                paint.set_anti_alias(false);
                paint.set_style(paint::Style::Stroke);

                let current_text: String = self.text[..self.selection.first].iter().collect();
                let current_text_width = ui.borrow_font().measure_str(current_text, None).0;

                let x = current_text_width + 1.0;
                let y1 = Self::height(ui) * 0.2;
                let y2 = Self::height(ui) * 0.8;
                canvas.draw_line((x, y1), (x, y2), &paint);
            });
        }

        if self.selection.first != self.selection.second {
            ui.draw_on_canvas(canvas, |canvas| {
                let mut paint = Paint::new(Color4f::from(colors.selection), None);
                paint.set_anti_alias(true);

                // Get all text from textfield start to current selection cursor position.
                // This will act as base position for selection.
                let selection_anchor_text: String = self.text[..self.selection.start()].iter().collect();
                let selection_anchor_text_width = ui.borrow_font().measure_str(selection_anchor_text, None).0;

                // Get text left to right or right to left depending on end cursor position.
                let selection_text: String = self.text[self.selection.normalize()].iter().collect();
                let selection_text_width = ui.borrow_font().measure_str(selection_text, None).0;

                let rrect = RRect::new_rect_xy(
                    &Rect::from_point_and_size(
                        (selection_anchor_text_width.round(), (Self::height(ui) * 0.2).round()),
                        (selection_text_width.round(), (Self::height(ui) * 0.6).round()),
                    ),
                    0.0,
                    0.0,
                );
                canvas.draw_rrect(rrect, &paint);
            });
        }

        canvas.restore();
        ui.pop_group();

        // Process events
        self.process_events(ui, input);

        ui.pop_group();
    }

    // Get selection content
    fn selection_text(&self) -> String {
        if self.selection.len() == 0 {
            return String::new()
        }

        self.text[self.selection.normalize()].iter().collect()
    }

    // Set text
    fn set_text(&mut self, text: String) {
        self.text = text.chars().collect();
        self.update_utf8();

        self.selection.move_cursors(self.text.len());
    }

    /// Resets the text field's blink timer.
    fn reset_blink(&mut self, input: &Input) {
        self.blink_start = input.time_in_seconds();
    }

    /// Appends a character to the cursor position.
    /// Or replaces selection if any.
    fn append(&mut self, ch: char) {
        if self.selection.len() > 0 {
            self.text.splice(self.selection.normalize(), vec![ch]);

            self.selection.move_cursors(self.selection.start() + 1);
        } else {
            self.text.insert(self.selection.first, ch);
            self.selection.move_cursors_right();
        }

        self.update_utf8();
    }

    /// Removes a character at cursor position.
    /// Or removes selection if any.
    fn backspace(&mut self) {
        if self.selection.len() != 0 {
            self.delete();
        } else if self.selection.first > 0 {
            self.selection.move_cursors_left();
            self.text.remove(self.selection.first);
        }

        self.update_utf8();
    }

    /// Removes character after cursor position.
    /// Or selection if any.
    fn delete(&mut self) {
        if self.selection.len() != 0 {
            self.text.drain(self.selection.normalize());
            self.selection.move_cursors(self.selection.start());
        } else if self.selection.first != self.text.len() {
            self.text.remove(self.selection.first);
        }

        self.update_utf8();
    }

    fn key_ctrl_down(&self, input: &Input) -> bool {
        input.key_is_down(VirtualKeyCode::LControl) || input.key_is_down(VirtualKeyCode::RControl)
    }

    fn key_shift_down(&self, input: &Input) -> bool {
        input.key_is_down(VirtualKeyCode::LShift) || input.key_is_down(VirtualKeyCode::RShift)
    }

    fn handle_word_skipping_and_selection(
        &mut self,
        input: &Input,
        not_found_ws_value: usize,
        range: Range<usize>,
        right: bool,
    ) {
        let mut found_ws = false;

        if right {
            let mut ix = self.selection.first;

            for char in self.text[range].iter() {
                ix += 1;

                if char.is_whitespace() {
                    self.selection.first = ix - 1;
                    if !self.key_shift_down(input) {
                        self.selection.second = self.selection.first;
                    }
                    found_ws = true;
                    break
                }
            }
        } else {
            let mut ix = self.selection.first;

            for char in self.text[range].iter().rev() {
                ix -= 1;

                if char.is_whitespace() {
                    self.selection.first = ix + 1;
                    if !self.key_shift_down(input) {
                        self.selection.second = self.selection.first;
                    }
                    found_ws = true;
                    break
                }
            }
        }

        if !found_ws {
            self.selection.first = not_found_ws_value;

            if !self.key_shift_down(input) {
                self.selection.second = self.selection.first;
            }
        }
    }

    /// Processes input events.
    fn process_events(&mut self, ui: &Ui, input: &Input) {
        if input.mouse_button_just_pressed(MouseButton::Left) {
            self.focused = ui.has_mouse(input);
            if self.focused {
                self.reset_blink(input);
            }
        }
        if self.focused {
            if !input.characters_typed().is_empty() {
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::Left) {
                if self.selection.first > 0 {
                    self.reset_blink(input);
                    self.selection.first -= 1;
                }

                if !self.key_shift_down(input) {
                    self.selection.second = self.selection.first;
                }

                if self.key_ctrl_down(input) {
                    self.handle_word_skipping_and_selection(input, 0, 0..self.selection.first, false);
                }
            }

            if input.key_just_typed(VirtualKeyCode::Right) {
                if self.selection.first < self.text.len() {
                    self.reset_blink(input);
                    self.selection.first += 1;
                }

                if !self.key_shift_down(input) {
                    self.selection.second = self.selection.first;
                }

                if self.key_ctrl_down(input) {
                    self.handle_word_skipping_and_selection(
                        input,
                        self.text.len(),
                        self.selection.first..self.text.len(),
                        true,
                    );
                }
            }

            if input.key_just_typed(VirtualKeyCode::Delete) {
                self.delete();
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::Home) {
                self.selection.move_cursors(0);
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::End) {
                self.selection.move_cursors(self.text.len());
                self.reset_blink(input);
            }

            if self.key_ctrl_down(input) {
                if input.key_just_typed(VirtualKeyCode::A) {
                    self.selection.second = 0;
                    self.selection.first = self.text.len();
                }

                if input.key_just_typed(VirtualKeyCode::C) {
                    self.clipboard_context.set_contents(self.selection_text()).unwrap();
                }

                if input.key_just_typed(VirtualKeyCode::V) {
                    let content = self.clipboard_context.get_contents();

                    if content.is_ok() {
                        if self.selection.len() > 0 {
                            let mut new_text: String = self.text.iter().collect();
                            new_text = new_text.replace(self.selection_text().as_str(), content.unwrap().as_str());

                            self.set_text(new_text);
                        } else {
                            let mut new_text: String = self.text.iter().collect();
                            new_text.push_str(content.unwrap().as_str());

                            self.set_text(new_text);
                        }
                    }
                }

                if input.key_just_typed(VirtualKeyCode::X) {
                    self.clipboard_context.set_contents(self.selection_text()).unwrap();
                    self.set_text("".to_owned());
                }
            }

            for ch in input.characters_typed() {
                match *ch {
                    _ if !ch.is_control() => self.append(*ch),
                    Self::BACKSPACE => self.backspace(),
                    _ => (),
                }
            }
        }
    }

    /// Returns the height of a labelled text field.
    pub fn labelled_height(ui: &Ui) -> f32 {
        16.0 + TextField::height(ui)
    }

    /// Processes a text field with an extra label above it.
    pub fn with_label(&mut self, ui: &mut Ui, canvas: &mut Canvas, input: &Input, label: &str, args: TextFieldArgs) {
        ui.push_group((args.width, Self::labelled_height(ui)), Layout::Vertical);

        // label
        ui.push_group((args.width, 16.0), Layout::Freeform);
        ui.text(canvas, label, args.colors.label, (AlignH::Left, AlignV::Top));
        ui.pop_group();

        // field
        self.process(ui, canvas, input, args);

        ui.pop_group();
    }

    /// Returns the text in the text field.
    pub fn text<'a>(&'a self) -> &'a str {
        &self.text_utf8
    }
}

impl Focus for TextField {
    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}
