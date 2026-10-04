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

### `src/adb.rs` (Role: general, Lines: 146)
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
  ```

### `src/bot/api.rs` (Role: api, Lines: 116)
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

### `src/bot/ui.rs` (Role: tui, Lines: 174)
- **Responsibility**: Core tui logic in src/bot/ui.rs
- **Imports**: use serde_json :: json , use crate :: tv :: TvStatus 
- **Public Functions & Signatures**:
  ```rust
  fn help_text (friendly_name : & str) -> String
  fn format_status_card (friendly_name : & str , status : & TvStatus) -> String
  fn status_keyboard (package : & str , app_name : & str) -> serde_json :: Value
  fn kill_keyboard () -> serde_json :: Value
  fn clear_keyboard () -> serde_json :: Value
  fn remote_keyboard () -> serde_json :: Value
  ```

### `src/bot.rs` (Role: general, Lines: 355)
- **Responsibility**: Core general logic in src/bot.rs
- **Imports**: use anyhow :: { Context , Result } , use reqwest :: blocking :: Client , use std :: time :: Duration , use crate :: config :: TerebiConfig , use crate :: tv :: TvController , use api :: { answer_callback , fetch_updates , send_photo , send_text , CallbackQuery , Message , User } , use ui :: { clear_keyboard , format_status_card , help_text , kill_keyboard , remote_keyboard , status_keyboard , } 
- **Public Functions & Signatures**:
  ```rust
  fn run_bot (config : & TerebiConfig) -> Result < () >
  ```

### `src/config.rs` (Role: general, Lines: 136)
- **Responsibility**: Core general logic in src/config.rs
- **Imports**: use anyhow :: { Context , Result } , use colored :: Colorize , use serde :: { Deserialize , Serialize } , use std :: fs , use std :: io :: { self , Write } , use std :: path :: { Path , PathBuf } 
- **Types & Enums**:
  ```rust
  pub struct TerebiConfig
  ```
- **Public Functions & Signatures**:
  ```rust
  fn config_path () -> PathBuf
  fn load () -> Result < Self >
  fn save (& self) -> Result < () >
  fn prompt_setup () -> Result < Self >
  ```

### `src/discovery.rs` (Role: general, Lines: 80)
- **Responsibility**: Core general logic in src/discovery.rs
- **Imports**: use std :: net :: { IpAddr , SocketAddr , TcpStream , UdpSocket } , use std :: process :: Command , use std :: sync :: mpsc , use std :: thread , use std :: time :: Duration 
- **Public Functions & Signatures**:
  ```rust
  fn find_attached_adb_device () -> Option < String >
  fn detect_local_subnet_prefix () -> Option < String >
  fn scan_subnet_for_adb (port : u16) -> Option < String >
  fn discover_tv_ip (port : u16) -> Option < String >
  ```

### `src/main.rs` (Role: general, Lines: 223)
- **Responsibility**: Core general logic in src/main.rs
- **Imports**: use anyhow :: Result , use clap :: { Parser , Subcommand } , use colored :: Colorize , use config :: TerebiConfig , use tv :: TvController 

### `src/media.rs` (Role: general, Lines: 274)
- **Responsibility**: Core general logic in src/media.rs
- **Types & Enums**:
  ```rust
  pub struct AppInfo
  pub struct MediaMetadata
  ```
- **Public Functions & Signatures**:
  ```rust
  fn get_app_display (package : & str) -> (& 'static str , & 'static str)
  fn resolve_package (input : & str) -> Option < String >
  fn parse_focused_package (window_dump : & str) -> Option < String >
  fn parse_media_session (media_dump : & str) -> MediaMetadata
  ```

### `src/tv.rs` (Role: general, Lines: 158)
- **Responsibility**: Core general logic in src/tv.rs
- **Imports**: use anyhow :: Result , use crate :: adb :: AdbDevice , use crate :: media :: { self , get_app_display , MediaMetadata } 
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
  fn take_screenshot (& self) -> Result < Vec < u8 > >
  fn send_key (& self , keycode : & str) -> Result < () >
  fn launch_app (& self , query : & str) -> Result < String >
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
