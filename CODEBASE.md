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

### `src/adb.rs` (Role: general, Lines: 237)
- **Responsibility**: Core general logic in src/adb.rs
- **Imports**: use anyhow :: { Context , Result } , use std :: process :: Command , use std :: sync :: { Arc , Mutex } , use std :: time :: Duration , use crate :: discovery 
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
  fn exec_raw (& self , args : & [& str]) -> Result < Vec < u8 > >
  const ADB_TIMEOUT : Duration = 20s (per-call hard deadline)
  const ADB_TIMEOUT : Duration = 20s (per-call hard deadline)
  ```

### `src/bot/api.rs` (Role: api, Lines: 127)
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
  fn answer_callback (client : & Client , token : & str , cb_id : & str , text : & str) -> Result < () >
  fn send_photo (client : & Client , token : & str , chat_id : i64 , caption : & str , image_bytes : Vec < u8 > ,) -> Result < () >
  ```

### `src/apps.rs` (Role: domain, Lines: 233)
- **Responsibility**: Installed-package listing, usage ranking, pagination helpers
- **Imports**: use std :: cmp :: Ordering , use std :: collections :: HashMap , use crate :: media :: get_app_display
- **Constants**: `PAGE_SIZE: usize = 10`
- **Types & Enums**:
  ```rust
  pub struct UsageRank { pub last_used: String, pub launches: u64 }
  ```
- **Public Functions & Signatures**:
  ```rust
  fn parse_package_list (output : & str) -> Vec < String >
  fn page_count (total : usize , per_page : usize) -> usize
  fn page_slice (items : & [String] , page : usize) -> (usize , & [String])
  fn display_name (package : & str) -> String
  fn parse_usage_ranking (dump : & str) -> HashMap < String , UsageRank >
  fn sort_by_usage (apps : Vec < String > , usage : & HashMap < String , UsageRank >) -> Vec < String >
  ```

### `src/bot/clear.rs` (Role: domain, Lines: 195)
- **Responsibility**: App-data wipe flows: installed-app menu, confirmation, execution
- **Imports**: use anyhow :: Result , use crate :: apps :: display_name , use crate :: bot :: api :: { answer_callback , send_text , CallbackQuery } , use crate :: bot :: ui :: { clear_apps_keyboard , confirm_clear_keyboard }
- **Public Functions & Signatures**:
  ```rust
  fn refresh_app_list (ctx : & BotContext) -> Result < usize >
  fn send_clear_menu (ctx : & BotContext , chat_id : i64)
  fn handle_clear_command (ctx : & BotContext , chat_id : i64 , input : & str)
  fn handle_clear_list (ctx : & BotContext , cb : & CallbackQuery , page : usize)
  fn handle_clear_refresh (ctx : & BotContext , cb : & CallbackQuery)
  fn handle_clear_ask (ctx : & BotContext , cb : & CallbackQuery , target : & str)
  fn handle_clear_execute (ctx : & BotContext , cb : & CallbackQuery , target : & str)
  ```
- **Callback Protocol**: `clearlist:<page>`, `clearrefresh`, `clearask:<index|package>`, `clear:<package>` (confirm always encodes the package, never an index)

### `src/input_log.rs` (Role: infra, Lines: 366)
- **Responsibility**: Remote-key capture from the WindowManager logcat channel, daily-file rotation, log tailing
- **Imports**: anyhow, std::fs, std::io, std::path, std::process, std::sync (Arc, Mutex, AtomicBool), std::thread, crate::adb
- **Types & Enums**:
  ```rust
  pub struct KeyEntry // date, time, key, action
  struct DailyWriter
  pub struct InputLogger
  ```
- **Public Functions & Signatures**:
  ```rust
  fn to_line (& self) -> String
  pub fn parse_key_line (line : & str) -> Option < KeyEntry >
  fn start (adb : & AdbDevice , dir : & Path) -> Result < Self >
  fn is_running (& self) -> bool
  fn stop (& mut self)
  pub fn latest_log (dir : & Path) -> Option < PathBuf >
  pub fn tail (path : & Path , count : usize) -> Result < Vec < String > >
  ```

### `src/bot/logs.rs` (Role: api, Lines: 201)
- **Responsibility**: Telegram handlers for key-capture control, log tailing and on-screen text
- **Imports**: anyhow::Result, serde_json::json, crate::bot (api, BotContext), crate::input_log
- **Public Functions & Signatures**:
  ```rust
  fn escape_html (text : & str) -> String
  fn send_log_tail (ctx : & BotContext , chat_id : i64)
  fn handle_log_toggle (ctx : & BotContext , chat_id : i64 , arg : Option < & str >)
  fn start_capture (ctx : & BotContext , chat_id : i64)
  fn stop_capture (ctx : & BotContext , chat_id : i64)
  fn send_capture_status (ctx : & BotContext , chat_id : i64)
  fn send_screen_text (ctx : & BotContext , chat_id : i64)
  ```

### `src/bot/ui.rs` (Role: tui, Lines: 392)
- **Responsibility**: Core tui logic in src/bot/ui.rs
- **Imports**: use serde_json :: json , use crate :: apps :: { display_name , page_count , page_slice } , use crate :: session :: format_duration , use crate :: tv :: TvStatus
- **Public Functions & Signatures**:
  ```rust
  fn help_text (friendly_name : & str) -> String
  fn format_status_card (friendly_name : & str , status : & TvStatus) -> String
  fn status_keyboard (package : & str , app_name : & str) -> serde_json :: Value
  fn kill_keyboard () -> serde_json :: Value
  fn clear_apps_keyboard (apps : & [String] , page : usize) -> serde_json :: Value
  fn confirm_clear_keyboard (target : & str) -> serde_json :: Value
  fn remote_keyboard () -> serde_json :: Value
  ```

### `src/bot.rs` (Role: general, Lines: 330)
- **Responsibility**: Core general logic in src/bot.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: cell :: RefCell , use std :: time :: Duration , use crate :: config :: TerebiConfig , use crate :: tv :: TvController , use api :: { answer_callback , fetch_updates , send_photo , send_text , CallbackQuery , Message , User } , use ui :: { format_status_card , help_text , kill_keyboard , remote_keyboard , status_keyboard , } 
- **Types & Enums**:
  ```rust
  struct BotContext < 'a > { client : & 'a Client , config : & 'a TerebiConfig , tv : & 'a TvController , apps : RefCell < Vec < String > > }
  ```
- **Public Functions & Signatures**:
  ```rust
  fn run_bot (config : & TerebiConfig) -> Result < () >
  ```

### `src/config.rs` (Role: general, Lines: 204)
- **Responsibility**: Config load/save and interactive setup wizard; honors `XDG_CONFIG_HOME` and backs up the previous file on overwrite
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use serde :: { Deserialize , Serialize } , use std :: fs , use std :: io :: { self , Write } , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct LogConfig
  pub struct TerebiConfig
  ```
- **Public Functions & Signatures**:
  ```rust
  fn path (& self) -> PathBuf            // LogConfig, expands leading ~/
  fn config_path () -> PathBuf           // $XDG_CONFIG_HOME else ~/.config
  fn load () -> Result < Self >
  fn save (& self) -> Result < () >      // copies to config.json.bak first
  fn prompt_setup () -> Result < Self >
  ```</｜｜DSML｜｜>

### `src/discovery.rs` (Role: general, Lines: 119)
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

### `src/main.rs` (Role: general, Lines: 314)
- **Responsibility**: CLI entrypoint, subcommand dispatch, colored terminal output
- **Imports**: use anyhow :: Result , use clap :: { Parser , Subcommand } , use colored :: Colorize , use config :: TerebiConfig , use tv :: TvController
- **Subcommands**: `daemon`, `status`, `screen`, `kill`, `clear`, `apps`, `open`, `remote`, `connect`, `logs`, `text`, `setup` 

### `src/media.rs` (Role: general, Lines: 229)
- **Responsibility**: Core general logic in src/media.rs
- **Types & Enums**:
  ```rust
  pub struct AppInfo
  ```
- **Public Functions & Signatures**:
  ```rust
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

### `src/tv.rs` (Role: general, Lines: 243)
- **Responsibility**: Core general logic in src/tv.rs
- **Imports**: use anyhow :: Result , use crate :: adb :: AdbDevice , use crate :: media :: { self , get_app_display } , use crate :: session :: { self , MediaMetadata } 
- **Types & Enums**:
  ```rust
  pub struct TvStatus
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
  fn send_key (& self , keycode : & str) -> Result < () >
  fn launch_app (& self , query : & str) -> Result < String >
  pub fn parse_ui_texts (xml : & str) -> Vec < String >
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
