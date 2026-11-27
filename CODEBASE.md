# CODEBASE.md: terebi Semantic Digest

> **Notice**: AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
Entrypoint ──> CLI/Parser ──> Domain Logic ──> Infra/IO
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language**: Rust 2021 edition
- **Architectural Paradigm**: Role-based (domain/, infra/, api/cli/, tui/)
- **Hard Constraints**: <400 lines/file, <60 lines/fn, zero production unwrap(), 0 warnings.
- **Target Distribution**: Linux x86_64 standalone binary

## 3. Module & Interface Skeleton

### `src/adb.rs` (Role: general, Lines: 321)
- **Responsibility**: Core general logic in src/adb.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs :: { self , File } , use std :: process :: { Command , Stdio } , use std :: sync :: atomic :: { AtomicU64 , Ordering } , use std :: sync :: { Arc , Mutex } , use std :: time :: { Duration , Instant } , use crate :: discovery 
- **Types & Enums**:
  ```rust
  pub struct AdbDevice
  ```
- **Public Functions & Signatures**:
  ```rust
  fn new (ip : String , port : u16) -> Self
  fn resolve_target (& self) -> Result < String >
  fn target (& self) -> String
  fn connect (& self) -> Result < bool >
  fn is_connected (& self) -> bool
  fn ensure_connected (& self) -> Result < () >
  fn shell (& self , args : & [& str]) -> Result < String >
  fn reboot (& self , power_off : bool) -> Result < () >
  fn exec_raw (& self , args : & [& str]) -> Result < Vec < u8 > >
  ```

### `src/apps.rs` (Role: general, Lines: 331)
- **Responsibility**: Core general logic in src/apps.rs
- **Imports**: use std :: cmp :: Ordering , use std :: collections :: HashMap , use crate :: media :: get_app_display 
- **Types & Enums**:
  ```rust
  pub struct UsageRank
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse_package_list (output : & str) -> Vec < String >
  fn parse_launchable_packages (output : & str) -> Vec < String >
  fn merge_package_lists (mut base : Vec < String > , extra : Vec < String >) -> Vec < String >
  fn parse_installed_users (dumpsys : & str) -> Vec < u32 >
  fn page_count (total : usize , per_page : usize) -> usize
  fn page_slice (items : & [String] , page : usize) -> (usize , & [String])
  fn display_name (package : & str) -> String
  fn parse_usage_ranking (dump : & str) -> HashMap < String , UsageRank >
  fn sort_by_usage (mut apps : Vec < String > , usage : & HashMap < String , UsageRank >) -> Vec < String >
  ```

### `src/bot/actions.rs` (Role: general, Lines: 139)
- **Responsibility**: Core general logic in src/bot/actions.rs
- **Imports**: use super :: api :: { answer_callback , escape_html , send_text , CallbackQuery } , use super :: ui :: { kill_keyboard , remote_keyboard } , use super :: BotContext 

### `src/bot/api.rs` (Role: api, Lines: 277)
- **Responsibility**: Core api logic in src/bot/api.rs
- **Imports**: use anyhow :: Result , use reqwest :: blocking :: multipart :: Part , use reqwest :: blocking :: { multipart , Client } , use serde :: Deserialize , use serde_json :: json 
- **Types & Enums**:
  ```rust
  pub struct TelegramResponse
  pub struct Update
  pub struct Message
  pub struct CallbackQuery
  pub struct MessageSummary
  pub struct User
  pub struct Chat
  ```
- **Public Functions & Signatures**:
  ```rust
  fn fetch_updates (client : & Client , token : & str , offset : i64) -> Result < Vec < Update > >
  fn send_text (client : & Client , token : & str , chat_id : i64 , text : & str , reply_markup : Option < serde_json :: Value > ,) -> Result < () >
  fn send_text_returning_id (client : & Client , token : & str , chat_id : i64 , text : & str , reply_markup : Option < serde_json :: Value > ,) -> Result < i64 >
  fn clear_inline_keyboard (client : & Client , token : & str , chat_id : i64 , message_id : i64 ,) -> Result < () >
  fn edit_message_text (client : & Client , token : & str , chat_id : i64 , message_id : i64 , text : & str , reply_markup : Option < serde_json :: Value > ,) -> Result < () >
  fn escape_html (text : & str) -> String
  fn truncate_utf16 (text : & str , max : usize) -> String
  fn answer_callback (client : & Client , token : & str , cb_id : & str , text : & str) -> Result < () >
  fn set_my_commands (client : & Client , token : & str , commands : & serde_json :: Value) -> Result < () >
  fn send_photo (client : & Client , token : & str , chat_id : i64 , caption : & str , image_bytes : Vec < u8 > ,) -> Result < () >
  ```

### `src/bot/clear/commands.rs` (Role: general, Lines: 222)
- **Responsibility**: Core general logic in src/bot/clear/commands.rs
- **Imports**: use crate :: bot :: api :: { edit_message_text , escape_html , send_text , send_text_returning_id } , use crate :: bot :: ui :: { wipe_delay_keyboard , wipe_scheduled_keyboard } , use super :: super :: BotContext , use super :: { confirm_target , delay_prompt_text , format_delay_label , resolve_command_target , send_clear_menu , ScheduleArgs , } 

### `src/bot/clear/scheduler.rs` (Role: general, Lines: 234)
- **Responsibility**: Core general logic in src/bot/clear/scheduler.rs
- **Imports**: use std :: sync :: atomic :: { AtomicBool , AtomicU64 , Ordering } , use std :: sync :: { Arc , Condvar , Mutex } , use std :: time :: { Duration , Instant } , use crate :: bot :: api :: { clear_inline_keyboard , escape_html , send_text } , use crate :: tv :: TvController 
- **Types & Enums**:
  ```rust
  pub struct ScheduleArgs
  pub struct ScheduledWipe
  pub struct WipeScheduler
  ```
- **Public Functions & Signatures**:
  ```rust
  fn new () -> Self
  fn schedule (self : & Arc < Self > , args : ScheduleArgs) -> u64
  fn cancel (& self , id : u64) -> Option < (String , String) >
  fn cancel_all (& self) -> Vec < (String , String) >
  fn list (& self) -> Vec < (u64 , String , String , u64) >
  ```

### `src/bot/clear.rs` (Role: general, Lines: 347)
- **Responsibility**: Core general logic in src/bot/clear.rs
- **Imports**: use anyhow :: Result , use crate :: apps :: display_name , use crate :: bot :: api :: { answer_callback , edit_message_text , escape_html , send_text , CallbackQuery } , use crate :: bot :: ui :: { cancelled_keyboard , clear_apps_keyboard , wipe_delay_keyboard , wipe_scheduled_keyboard , } , use super :: BotContext , pub (super) use commands :: handle_clear_command , pub use scheduler :: { ScheduleArgs , WipeScheduler } 

### `src/bot/commands.rs` (Role: general, Lines: 140)
- **Responsibility**: Core general logic in src/bot/commands.rs
- **Imports**: use serde_json :: json 
- **Public Functions & Signatures**:
  ```rust
  fn help_text (friendly_name : & str) -> String
  fn command_menu () -> serde_json :: Value
  ```

### `src/bot/logs.rs` (Role: general, Lines: 263)
- **Responsibility**: Core general logic in src/bot/logs.rs
- **Imports**: use crate :: input_log :: { latest_log , tail , InputLogger } , use super :: api :: { escape_html , send_text } , use super :: BotContext 

### `src/bot/power.rs` (Role: general, Lines: 220)
- **Responsibility**: Core general logic in src/bot/power.rs
- **Imports**: use std :: time :: { Duration , Instant } , use crate :: bot :: api :: { answer_callback , edit_message_text , escape_html , send_text , CallbackQuery , Message , } , use crate :: bot :: ui :: { cancelled_keyboard , power_keyboard , PowerAction } , use crate :: tv :: PowerOutcome , use super :: BotContext 

### `src/bot/ui.rs` (Role: tui, Lines: 376)
- **Responsibility**: Core tui logic in src/bot/ui.rs
- **Imports**: use serde_json :: json , use crate :: apps :: { display_name , page_count , page_slice } , use crate :: bot :: api :: escape_html , use crate :: session :: format_duration , use crate :: tv :: TvStatus 
- **Types & Enums**:
  ```rust
  pub enum PowerAction
  ```
- **Public Functions & Signatures**:
  ```rust
  fn format_status_card (friendly_name : & str , status : & TvStatus) -> String
  fn status_keyboard (package : & str , app_name : & str) -> serde_json :: Value
  fn kill_keyboard () -> serde_json :: Value
  fn clear_apps_keyboard (apps : & [String] , page : usize) -> serde_json :: Value
  fn wipe_delay_keyboard (target : & str) -> serde_json :: Value
  fn wipe_scheduled_keyboard (id : u64) -> serde_json :: Value
  fn from_command (command : & str) -> Option < Self >
  fn callback (self) -> & 'static str
  fn title (self) -> & 'static str
  fn icon (self) -> & 'static str
  fn power_keyboard (action : PowerAction) -> serde_json :: Value
  fn cancelled_keyboard () -> serde_json :: Value
  fn remote_keyboard () -> serde_json :: Value
  ```

### `src/bot.rs` (Role: general, Lines: 300)
- **Responsibility**: Core general logic in src/bot.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: cell :: RefCell , use std :: sync :: Arc , use std :: time :: { Duration , Instant } , use crate :: config :: TerebiConfig , use crate :: input_log :: InputLogger , use crate :: tv :: TvController , use actions :: { handle_kill_callback , handle_kill_command , handle_open_command , send_remote_menu } , use api :: { answer_callback , escape_html , fetch_updates , send_photo , send_text , set_my_commands , CallbackQuery , Message , User , } , use ui :: { format_status_card , status_keyboard , PowerAction } 
- **Public Functions & Signatures**:
  ```rust
  fn run_bot (config : & TerebiConfig) -> Result < () >
  ```

### `src/config.rs` (Role: general, Lines: 217)
- **Responsibility**: Core general logic in src/config.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use serde :: { Deserialize , Serialize } , use std :: fs , use std :: io :: { self , Write } , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct LogConfig
  pub struct TerebiConfig
  ```
- **Public Functions & Signatures**:
  ```rust
  fn path (& self) -> PathBuf
  fn config_path () -> PathBuf
  fn load () -> Result < Self >
  fn save (& self) -> Result < () >
  fn prompt_setup () -> Result < Self >
  ```

### `src/discovery.rs` (Role: general, Lines: 129)
- **Responsibility**: Core general logic in src/discovery.rs
- **Imports**: use std :: net :: { IpAddr , SocketAddr , TcpStream , UdpSocket } , use std :: process :: Command , use std :: sync :: mpsc , use std :: thread , use std :: time :: Duration 
- **Public Functions & Signatures**:
  ```rust
  fn is_emulator_serial (serial : & str) -> bool
  fn find_attached_adb_device () -> Option < String >
  fn detect_local_subnet_prefix () -> Option < String >
  fn scan_subnet_for_adb (port : u16) -> Option < String >
  fn discover_tv_ip (port : u16) -> Option < String >
  ```

### `src/input_log/capture.rs` (Role: general, Lines: 282)
- **Responsibility**: Core general logic in src/input_log/capture.rs
- **Imports**: use std :: fs :: { self , File , OpenOptions } , use std :: io :: { BufRead , BufReader , Write } , use std :: path :: { Path , PathBuf } , use std :: process :: { Child , Command , Stdio } , use std :: sync :: atomic :: { AtomicBool , Ordering } , use std :: sync :: { Arc , Mutex } , use std :: thread :: { self , JoinHandle } , use std :: time :: Duration , use anyhow :: { Context , Result } , use crate :: adb :: AdbDevice , use super :: { parse_key_line , KeyEntry } 
- **Types & Enums**:
  ```rust
  pub struct InputLogger
  ```
- **Public Functions & Signatures**:
  ```rust
  fn start (adb : & AdbDevice , dir : & Path) -> Result < Self >
  fn is_running (& self) -> bool
  fn is_capturing (& self) -> bool
  fn stop (& mut self)
  ```

### `src/input_log.rs` (Role: general, Lines: 221)
- **Responsibility**: Core general logic in src/input_log.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: fs , use std :: path :: { Path , PathBuf } , pub use capture :: InputLogger 
- **Types & Enums**:
  ```rust
  pub struct KeyEntry
  ```
- **Public Functions & Signatures**:
  ```rust
  fn to_line (& self) -> String
  fn parse_key_line (line : & str) -> Option < KeyEntry >
  fn latest_log (dir : & Path) -> Option < PathBuf >
  fn tail (path : & Path , count : usize) -> Result < Vec < String > >
  ```

### `src/main.rs` (Role: general, Lines: 358)
- **Responsibility**: Core general logic in src/main.rs
- **Imports**: use anyhow :: Result , use clap :: { Parser , Subcommand } , use colored :: Colorize , use config :: TerebiConfig , use tv :: TvController 

### `src/media/catalog.rs` (Role: general, Lines: 270)
- **Responsibility**: Core general logic in src/media/catalog.rs
- **Types & Enums**:
  ```rust
  pub struct AppInfo
  ```

### `src/media.rs` (Role: general, Lines: 165)
- **Responsibility**: Core general logic in src/media.rs
- **Imports**: pub use catalog :: KNOWN_APPS 
- **Public Functions & Signatures**:
  ```rust
  fn is_valid_package_id (package : & str) -> bool
  fn is_valid_keycode (keycode : & str) -> bool
  fn get_app_display (package : & str) -> (& 'static str , & 'static str)
  fn resolve_package (input : & str) -> Option < String >
  fn parse_focused_package (window_dump : & str) -> Option < String >
  ```

### `src/session.rs` (Role: general, Lines: 196)
- **Responsibility**: Core general logic in src/session.rs
- **Types & Enums**:
  ```rust
  pub struct MediaMetadata
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse_media_session (media_dump : & str , focused_package : Option < & str >) -> MediaMetadata
  fn format_duration (ms : u64) -> String
  ```

### `src/tv.rs` (Role: general, Lines: 321)
- **Responsibility**: Core general logic in src/tv.rs
- **Imports**: use anyhow :: Result , use crate :: adb :: AdbDevice , use crate :: media :: { self , get_app_display } , use crate :: session :: { self , MediaMetadata } 
- **Types & Enums**:
  ```rust
  pub struct TvStatus
  pub enum PowerOutcome
  pub struct TvController
  ```
- **Public Functions & Signatures**:
  ```rust
  fn new (ip : String , port : u16) -> Self
  fn get_status (& self) -> Result < TvStatus >
  fn force_stop (& self , query : & str) -> Result < (String , String) >
  fn clear_app_data (& self , query : & str) -> Result < (String , String) >
  fn list_apps (& self) -> Result < Vec < String > >
  fn take_screenshot (& self) -> Result < Vec < u8 > >
  fn screen_texts (& self) -> Result < Vec < String > >
  fn reboot (& self) -> Result < PowerOutcome >
  fn power_off (& self) -> Result < PowerOutcome >
  fn send_key (& self , keycode : & str) -> Result < () >
  fn launch_app (& self , query : & str) -> Result < String >
  fn parse_ui_texts (xml : & str) -> Vec < String >
  ```

## 4. Execution Lifecycle Trace
1. **Startup**: Entrypoint parses CLI flags & dispatches command.
2. **Execution**: Core domain logic processes inputs and evaluates rules.
3. **Persistence / I/O**: Domain logic calls infra for disk/terminal I/O.
4. **Exit**: Graceful termination with standard exit codes.

## 5. Verification Commands
```bash
cargo build --release --target x86_64-unknown-linux-gnu
cargo test --all-targets
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```
