//! User configuration.
//!
//! ## Note for adding new keys
//!
//! New keys added to the config _must_ use `#[serde(default)]` to maintain compatibility with
//! older configs. These keys will be added to the user's configuration automatically.

use std::path::PathBuf;
use std::{fmt, str};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Saved values of lobby text boxes.
#[derive(Deserialize, Serialize)]
pub struct LobbyConfig {
   pub nickname: String,
   pub matchmaker: String,
}

/// The color scheme variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ColorScheme {
   Light,
   Dark,
}

// fmt::Display implements to_string() for us
impl fmt::Display for ColorScheme {
   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      write!(f, "{:?}", self)
   }
}

impl str::FromStr for ColorScheme {
   type Err = ();

   fn from_str(s: &str) -> Result<ColorScheme, ()> {
      let color = s.to_lowercase();
      match color.as_str() {
         "light" => Ok(Self::Light),
         "dark" => Ok(Self::Dark),
         _ => Err(()),
      }
   }
}

/// The position of the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ToolbarPosition {
   /// Vertical on the left side of the screen.
   Left,
   /// Horizontal on the top of the screen.
   Top,
   /// Vertical on the right side of the screen.
   Right,
   /// Horizontal on the bottom of the screen.
   Bottom,
}

impl Default for ToolbarPosition {
   /// The default toolbar position is the left-hand side of the screen.
   fn default() -> Self {
      Self::Left
   }
}

/// UI-related configuration options.
#[derive(Deserialize, Serialize)]
pub struct UiConfig {
   pub color_scheme: ColorScheme,
   #[serde(default)]
   pub toolbar_position: ToolbarPosition,
}

/// A user `config.toml` file.
#[derive(Deserialize, Serialize)]
pub struct UserConfig {
   pub lobby: LobbyConfig,
   pub ui: UiConfig,
}

impl UserConfig {
   /// Returns the platform-specific configuration directory.
   pub fn config_dir() -> PathBuf {
      let project_dirs =
         ProjectDirs::from("", "", "NetCanv").expect("cannot determine config directories");
      project_dirs.config_dir().to_owned()
   }

   /// Returns the path to the `config.toml` file.
   pub fn path() -> PathBuf {
      Self::config_dir().join("config.toml")
   }

   /// Loads the `config.toml` file.
   ///
   /// If the `config.toml` doesn't exist, it's created with values inherited from
   /// `UserConfig::default`.
   #[cfg(not(target_arch = "wasm32"))]
   pub fn load_or_create() -> anyhow::Result<Self> {
      let config_dir = Self::config_dir();
      let config_file = Self::path();
      std::fs::create_dir_all(config_dir)?;
      if !config_file.is_file() {
         let config = Self::default();
         config.save()?;
         Ok(config)
      } else {
         let file = std::fs::read_to_string(&config_file)?;
         let config: Self = match toml::from_str(&file) {
            Ok(config) => config,
            Err(error) => {
               eprintln!("error while deserializing config file: {}", error);
               eprintln!("falling back to default config");
               return Ok(Self::default());
            }
         };
         // Preemptively save the config to the disk if any new keys have been added.
         // I'm not sure if errors should be treated as fatal or not in this case.
         config.save()?;
         Ok(config)
      }
   }

   #[cfg(target_arch = "wasm32")]
   pub fn load_or_create() -> anyhow::Result<Self> {
      let window = web_sys::window().unwrap();
      let storage = window.local_storage().unwrap().unwrap();

      let mut config = Self::default();
      config.lobby.nickname = match storage.get_item("nickname").unwrap() {
         Some(i) => i,
         None => {
            storage.set_item("nickname", &config.lobby.nickname).unwrap();
            config.lobby.nickname
         }
      };

      config.lobby.matchmaker = match storage.get_item("matchmaker").unwrap() {
         Some(i) => i,
         None => {
            storage.set_item("matchmaker", &config.lobby.matchmaker).unwrap();
            config.lobby.matchmaker
         }
      };

      config.ui.color_scheme = match storage.get_item("color_scheme").unwrap() {
         Some(i) => i.parse::<ColorScheme>().unwrap(),
         None => {
            storage.set_item("color_scheme", &config.ui.color_scheme.to_string()).unwrap();
            config.ui.color_scheme
         }
      };

      Ok(config)
   }

   /// Saves the user configuration to the `config.toml` file.
   #[cfg(not(target_arch = "wasm32"))]
   pub fn save(&self) -> anyhow::Result<()> {
      // Assumes that `config_dir` was already created in `load_or_create`.
      let config_file = Self::path();
      std::fs::write(&config_file, toml::to_string(self)?)?;
      Ok(())
   }

   #[cfg(target_arch = "wasm32")]
   pub fn save(&self) -> anyhow::Result<()> {
      let window = web_sys::window().unwrap();
      let storage = window.local_storage().unwrap().unwrap();

      storage.set_item("nickname", &self.lobby.nickname).unwrap();
      storage.set_item("matchmaker", &self.lobby.matchmaker).unwrap();
      storage.set_item("color_scheme", &self.ui.color_scheme.to_string()).unwrap();

      Ok(())
   }
}

impl Default for UserConfig {
   fn default() -> Self {
      Self {
         lobby: LobbyConfig {
            nickname: "Anon".to_owned(),
            matchmaker: "localhost".to_owned(),
         },
         ui: UiConfig {
            color_scheme: ColorScheme::Light,
            toolbar_position: ToolbarPosition::Left,
         },
      }
   }
}
