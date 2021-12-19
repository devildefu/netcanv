#[cfg(not(target_arch = "wasm32"))]
fn main() {
   use log::LevelFilter;
   use native_dialog::{MessageDialog, MessageType};
   use simplelog::{Config, SimpleLogger};
   use std::fmt::Write;

   let _ = SimpleLogger::init(LevelFilter::Info, Config::default());

   let default_panic_hook = std::panic::take_hook();
   std::panic::set_hook(Box::new(move |panic_info| {
      // Pretty panic messages are only enabled in release mode, as they hinder debugging.
      #[cfg(not(debug_assertions))]
      {
         let mut message = heapless::String::<8192>::new();
         let _ = write!(message, "Oh no! A fatal error occured.\n{}", panic_info);
         let _ = write!(message, "\n\nThis is most definitely a bug, so please file an issue on GitHub. https://github.com/liquidev/netcanv");
         let _ = MessageDialog::new()
            .set_title("NetCanv - Fatal Error")
            .set_text(&message)
            .set_type(MessageType::Error)
            .show_alert();
      }
      default_panic_hook(panic_info);
   }));

   match netcanv::main() {
      Ok(()) => (),
      Err(payload) => {
         let mut message = String::new();
         let _ = write!(
            message,
            "An error occured:\n{}\n\nIf you think this is a bug, please file an issue on GitHub. https://github.com/liquidev/netcanv",
            payload
         );
         log::info!("main() returned with an Err:\n{}", payload);
         MessageDialog::new()
            .set_title("NetCanv - Error")
            .set_text(&message)
            .set_type(MessageType::Error)
            .show_alert()
            .unwrap();
      }
   }
}

// I don't know why, but rustc needs this
#[cfg(target_arch = "wasm32")]
fn main() {}
