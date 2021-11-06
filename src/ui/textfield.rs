//! A fairly simplistic text field implementation.

use std::ops::Range;

use copypasta::{ClipboardContext, ClipboardProvider};
use netcanv_renderer::Font as FontTrait;
use paws::{point, vector, AlignH, AlignV, Color, Layout, LineCap, Rect, Renderer};

use crate::{backend::Font, ui::*};

/// Text field selection.
/// Stores two cursors: the text cursor and the selection anchor.
/// These cursors are modified appropriately as the user edits text.
struct Selection {
   cursor: usize,
   anchor: usize,
}

impl Selection {
   pub fn start(&self) -> usize {
      self.cursor.min(self.anchor)
   }

   pub fn end(&self) -> usize {
      self.cursor.max(self.anchor)
   }

   pub fn normalize(&self) -> Range<usize> {
      self.start()..self.end()
   }

   pub fn len(&self) -> usize {
      self.end() - self.start()
   }

   pub fn move_to(&mut self, position: usize) {
      self.cursor = position;
      self.anchor = self.cursor;
   }

   pub fn move_left(&mut self, is_shift_down: bool) {
      if self.cursor > 0 {
         self.cursor -= 1;

         if !is_shift_down {
            self.anchor = self.cursor;
         }
      }
   }

   pub fn move_right(&mut self, is_shift_down: bool) {
      self.cursor += 1;

      if !is_shift_down {
         self.anchor = self.cursor;
      }
   }
}

enum ArrowKey {
   Left,
   Right,
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
pub struct TextFieldArgs<'a, 'b, 'c> {
   pub width: f32,
   pub colors: &'a TextFieldColors,
   pub hint: Option<&'b str>,
   pub font: &'c Font,
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
      let text: Vec<char> = text_utf8.chars().collect();
      let length = text.len();

      Self {
         text,
         text_utf8,
         focused: false,
         blink_start: 0.0,

         selection: Selection {
            cursor: length,
            anchor: length,
         },

         clipboard_context: ClipboardContext::new().unwrap(),
      }
   }

   /// Updates the text field's UTF-8 string.
   fn update_utf8(&mut self) {
      self.text_utf8 = self.text.iter().collect();
   }

   /// Returns the height of a text field.
   pub fn height(font: &Font) -> f32 {
      f32::round(16.0 / 7.0 * font.size())
   }

   /// Processes a text field.
   pub fn process(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      TextFieldArgs {
         font,
         width,
         colors,
         hint,
      }: TextFieldArgs,
   ) {
      ui.push((width, Self::height(font)), Layout::Freeform);

      // Rendering: box
      let outline_color = if self.focused {
         colors.outline_focus
      } else {
         colors.outline
      };
      ui.fill_rounded(colors.fill, 4.0);
      ui.outline_rounded(outline_color, 4.0, 1.0);

      // Rendering: text
      ui.push(ui.size(), Layout::Freeform);
      ui.pad((8.0, 0.0));
      ui.render().push();
      ui.clip();

      // Rendering: hint
      if hint.is_some() && self.text.len() == 0 {
         ui.text(
            font,
            hint.unwrap(),
            colors.text_hint,
            (AlignH::Left, AlignV::Middle),
         );
      }

      if !self.focused {
         self.selection.anchor = self.selection.cursor;
      }

      if self.focused
         && (input.time_in_seconds() - self.blink_start) % Self::BLINK_PERIOD < Self::HALF_BLINK
      {
         ui.draw(|ui| {
            let current_text: String = self.text[..self.selection.cursor].iter().collect();
            let current_text_width = font.text_width(&current_text);

            let x = current_text_width + 1.0;
            let y1 = Self::height(font) * 0.2;
            let y2 = Self::height(font) * 0.8;
            ui.line(point(x, y1), point(x, y2), colors.text, LineCap::Butt, 1.0);
         });
      }

      if self.selection.cursor != self.selection.anchor {
         ui.draw(|ui| {
            // Get all the text starting from the start of the textbox to the first position
            // of the selection.
            // From this, we can calculate where to position the selection rectangle.
            let selection_anchor_text: String =
               self.text[..self.selection.start()].iter().collect();
            let selection_anchor_text_width = font.text_width(&selection_anchor_text).round();

            // Get all the selected text and its width.
            let selection_text: String = self.text[self.selection.normalize()].iter().collect();
            let selection_text_width = font.text_width(&selection_text).round();

            ui.render().fill(
               Rect::new(
                  point(selection_anchor_text_width, Self::height(font) * 0.2),
                  vector(selection_text_width, Self::height(font) * 0.6),
               ),
               colors.selection,
               0.0,
            )
         });
      }

      ui.text(
         font,
         &self.text_utf8,
         colors.text,
         (AlignH::Left, AlignV::Middle),
      );

      ui.render().pop();
      ui.pop();

      // Process events
      self.process_events(ui, input);

      ui.pop();
   }

   // Get selection content
   fn selection_text(&self) -> String {
      if self.selection.len() == 0 {
         return String::new();
      }

      self.text[self.selection.normalize()].iter().collect()
   }

   // Set text
   fn set_text(&mut self, text: String) {
      self.text = text.chars().collect();
      self.update_utf8();

      self.selection.move_to(self.text.len());
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

         self.selection.move_to(self.selection.start() + 1);
      } else {
         self.text.insert(self.selection.cursor, ch);
         self.selection.move_right(false);
      }

      self.update_utf8();
   }

   /// Removes a character at cursor position.
   /// Or removes selection if any.
   fn backspace(&mut self) {
      if self.selection.len() != 0 {
         self.delete();
      } else if self.selection.cursor > 0 {
         self.selection.move_left(false);
         self.text.remove(self.selection.cursor);
      }

      self.update_utf8();
   }

   /// Removes character after cursor position.
   /// Or selection if any.
   fn delete(&mut self) {
      if self.selection.len() != 0 {
         self.text.drain(self.selection.normalize());
         self.selection.move_to(self.selection.start());
      } else if self.selection.cursor != self.text.len() {
         self.text.remove(self.selection.cursor);
      }

      self.update_utf8();
   }

   fn key_ctrl_down(&self, input: &Input) -> bool {
      input.key_is_down(VirtualKeyCode::LControl) || input.key_is_down(VirtualKeyCode::RControl)
   }

   fn key_shift_down(&self, input: &Input) -> bool {
      input.key_is_down(VirtualKeyCode::LShift) || input.key_is_down(VirtualKeyCode::RShift)
   }

   fn process_word_skipping_and_selection(
      &mut self,
      range: Range<usize>,
      arrow_key: ArrowKey,
      is_shift_down: bool,
   ) {
      let mut found_whitespace = false;
      let mut ix: usize = 0;

      let text_in_range = &self.text[range];

      let text_for_range: Vec<&char> = match arrow_key {
         ArrowKey::Right => text_in_range.iter().collect(),
         ArrowKey::Left => text_in_range.iter().rev().collect(),
      };

      let mut iter = text_for_range.iter().enumerate().peekable();

      while let Some((i, ch)) = iter.next() {
         let next_char = match iter.peek() {
            Some(next_ch) => next_ch.1,
            None => &' ',
         };

         if ch.is_whitespace() {
            ix = i;
            continue;
         }

         if next_char.is_whitespace() {
            found_whitespace = true;
            break;
         }

         ix = i + 1;
      }

      if found_whitespace {
         match arrow_key {
            ArrowKey::Right => self.selection.cursor += ix + 1,
            ArrowKey::Left => self.selection.cursor -= ix + 1,
         };

         if !is_shift_down {
            self.selection.anchor = self.selection.cursor;
         }
      } else {
         self.selection.cursor = match arrow_key {
            ArrowKey::Right => self.text.len(),
            ArrowKey::Left => 0,
         };

         if !is_shift_down {
            self.selection.anchor = self.selection.cursor;
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
            self.reset_blink(input);

            if self.key_ctrl_down(input) {
               self.process_word_skipping_and_selection(
                  0..self.selection.cursor,
                  ArrowKey::Left,
                  self.key_shift_down(input),
               );
            } else {
               self.selection.move_left(self.key_shift_down(input));
            }
         }

         if input.key_just_typed(VirtualKeyCode::Right) {
            self.reset_blink(input);

            if self.key_ctrl_down(input) {
               self.process_word_skipping_and_selection(
                  self.selection.cursor..self.text.len(),
                  ArrowKey::Right,
                  self.key_shift_down(input),
               );
            } else if self.selection.cursor < self.text.len() {
               self.selection.move_right(self.key_shift_down(input));
            }
         }

         if input.key_just_typed(VirtualKeyCode::Back) {
            self.backspace();
         }

         if input.key_just_typed(VirtualKeyCode::Delete) {
            self.delete();
            self.reset_blink(input);
         }

         if input.key_just_typed(VirtualKeyCode::Home) {
            self.selection.move_to(0);
            self.reset_blink(input);
         }

         if input.key_just_typed(VirtualKeyCode::End) {
            self.selection.move_to(self.text.len());
            self.reset_blink(input);
         }

         if self.key_ctrl_down(input) {
            if input.key_just_typed(VirtualKeyCode::A) {
               self.selection.anchor = 0;
               self.selection.cursor = self.text.len();
            }

            if input.key_just_typed(VirtualKeyCode::C) {
               self.clipboard_context.set_contents(self.selection_text()).unwrap();
            }

            if input.key_just_typed(VirtualKeyCode::V) {
               let content = self.clipboard_context.get_contents();

               if content.is_ok() {
                  if self.selection.len() > 0 {
                     let mut new_text: String = self.text.iter().collect();
                     new_text =
                        new_text.replace(self.selection_text().as_str(), content.unwrap().as_str());

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

            if input.key_just_typed(VirtualKeyCode::Back) {
               self.process_word_skipping_and_selection(
                  0..self.selection.cursor,
                  ArrowKey::Left,
                  true,
               );
            }

            if input.key_just_typed(VirtualKeyCode::Delete) {
               self.process_word_skipping_and_selection(
                  self.selection.cursor..self.text.len(),
                  ArrowKey::Right,
                  true,
               );

               self.delete();
            }
         }

         for ch in input.characters_typed() {
            match *ch {
               _ if !ch.is_control() => self.append(*ch),
               _ => (),
            }
         }
      }
   }

   /// Returns the height of a labelled text field.
   pub fn labelled_height(font: &Font) -> f32 {
      16.0 + TextField::height(font)
   }

   /// Processes a text field with an extra label above it.
   pub fn with_label(&mut self, ui: &mut Ui, input: &Input, label: &str, args: TextFieldArgs) {
      ui.push(
         (args.width, Self::labelled_height(args.font)),
         Layout::Vertical,
      );

      // label
      ui.push((args.width, 16.0), Layout::Freeform);
      ui.text(
         args.font,
         label,
         args.colors.label,
         (AlignH::Left, AlignV::Top),
      );
      ui.pop();

      // field
      self.process(ui, input, args);

      ui.pop();
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
