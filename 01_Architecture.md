# Doc 1: システムアーキテクチャ詳細仕様書 (Ver 6.0)
本ドキュメントは、`LightningFiler` の基盤となる技術仕様を定義する。
実装者は本仕様書に記載されたディレクトリ構造、クレート選定、メモリ管理方針、およびエラー処理フローに厳密に従うこと。

## 1. プロジェクト構成とビルド環境

### 1.1 ワークスペース構成 (Cargo Workspace)
Monorepo構成を採用し、コンパイル単位を分離することで、並列ビルドの効率化と依存関係の明確化を図る。

**ディレクトリツリー:**
```text
LightningFiler/
├── Cargo.toml [workspace]
├── .cargo/
│   └── config.toml (Windows特化リンカ設定)
├── crates/
│   ├── app_main/       # [Bin] 64bit エントリーポイント
│   │                   # - GUIイベントループ (winit)
│   │                   # - アプリケーションライフサイクル管理
│   │
│   ├── app_core/       # [Lib] ドメインロジック・状態管理
│   │                   # - AppState, GlobalConfig
│   │                   # - エラー定義 (AppError)
│   │                   # - コマンドディスパッチャ
│   │
│   ├── app_ui/         # [Lib] プレゼンテーション層
│   │                   # - egui コンポーネント
│   │                   # - wgpu レンダリングパイプライン
│   │                   # - キーバインド処理
│   │
│   ├── app_db/         # [Lib] データ永続化層
│   │                   # - SQLite (メタデータ)
│   │                   # - RocksDB (KVS / サムネイルキャッシュ)
│   │                   # - マイグレーションロジック
│   │
│   ├── app_fs/         # [Lib] ファイルシステム抽象化層
│   │                   # - VFS (Virtual File System)
│   │                   # - パス処理 (UniversalPath)
│   │                   # - ファイル監視 (notify)
│   │                   # - 文字コード判別 (chardet)
│   │
│   ├── app_log/        # [Lib] オブザーバビリティ基盤
│   │                   # - 構造化ロギング (tracing)
│   │                   # - パニックフック
│   │                   # - クラッシュレポート生成
│   │
│   ├── susie_host/     # [Bin] 32bit Susieプラグインホスト
│   │                   # - i686-pc-windows-msvc ターゲットでビルド
│   │                   # - SPIプラグインロード
│   │
│   └── ipc_proto/      # [Lib] プロセス間通信プロトコル
│                       # - 共有メモリレイアウト定義
│                       # - コマンド/レスポンス (bincode)
│
└── scripts/            # 開発支援スクリプト
    ├── build_release.ps1   # リリースビルド＆署名
    └── deploy_bridge.ps1   # 32bit Bridgeの配置
```

### 1.2 ルート `Cargo.toml` 設定
ワークスペース全体の共通設定と、最適化プロファイルを定義する。

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Your Name"]
rust-version = "1.75" # 非同期トレイト等の安定化バージョン以降

[workspace.dependencies]
# 非同期・並列処理
tokio = { version = "1.36", features = ["full", "tracing"] }
rayon = "1.9"
crossbeam-channel = "0.5"
parking_lot = "0.12" # 標準Mutexより高速・小型
dashmap = "5.5"      # 並列ハッシュマップ
backtrace = "0.3"     # Panic Hook の Backtrace
chrono    = "0.4"     # Panic Hook のタイムスタンプ
chardetng = "0.1"     # 書庫エンコーディング判別（zip展開時）

# GUI・描画
winit = "0.29"
egui = "0.26"
egui-wgpu = "0.26"
wgpu = "0.19"
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "webp", "bmp"] }

# システム・IPC
windows = { version = "0.52", features = ["Win32_System_Memory", "Win32_System_JobObjects", "Win32_UI_WindowsAndMessaging"] }
interprocess = "1.2"
shared_memory = "0.12"
bincode = "1.3"
serde = { version = "1.0", features = ["derive"] }

# ロギング・エラー
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-appender = "0.2"
thiserror = "1.0"
anyhow = "1.0"

# データ
rusqlite = { version = "0.31", features = ["bundled"] } # DLL依存排除
rocksdb = "0.21"
xxhash-rust = { version = "0.8", features = ["xxh3"] }

# プロファイル設定 (リリースビルドの最適化)
[profile.release]
lto = "fat"        # Link Time Optimization (最大)
codegen-units = 1  # 並列コンパイル無効化 (最適化優先)
panic = "abort"    # パニック時は即終了 (スタック巻き戻しなし)
strip = true       # デバッグシンボル削除
```

### 1.3 `.cargo/config.toml` (Windows特化設定)
再帰的なフォルダ探索や、巨大な構造体のスタック確保によるスタックオーバーフローを防ぐため、スタックサイズを拡張する。

```toml
[target.x86_64-pc-windows-msvc]
rustflags = [
    "-C", "target-feature=+crt-static", # MSVCランタイムを静的リンク (配布時のDLL地獄回避)
    "-C", "link-arg=/STACK:8388608",    # スタックサイズを8MBに拡張 (デフォルトは1MB)
]

[target.i686-pc-windows-msvc]
rustflags = [
    "-C", "target-feature=+crt-static",
    "-C", "link-arg=/STACK:4194304",    # 32bit側は4MB
]
```

---

## 2. オブザーバビリティとデバッグ基盤 (Deep Observability)

### 2.1 構造化ロギング (`app_log`)
`tracing` エコシステムを使用し、スレッドID、タイムスタンプ、モジュールパスを含む構造化ログを出力する。

*   **ログ出力仕様**:
    *   **開発時 (Debug)**: コンソールに `Pretty` フォーマットで出力。
    *   **本番時 (Release)**: `%APPDATA%\LightningFiler\logs\` に `JSON` 形式で出力（解析ツールでの読み込み用）。
    *   **非同期書き込み**: `tracing_appender::non_blocking` を使用し、ログ書き込みによるメインスレッドのI/Oブロックを完全に防ぐ。

*   **ログローテーション**:
    *   **単位**: 日次 (`app.YYYY-MM-DD.log`)。
    *   **保持期間**: 起動時に `app_log::cleanup_old_logs(days: 7)` を呼び出し、7日以上前のログを削除する。

### 2.2 パニックフックとクラッシュレポート (Panic Handler)
Rustの `panic!` 発生時、アプリケーションがただ落ちるのではなく、原因究明に必要な「検死情報」を残す。

```rust
use backtrace::Backtrace;
use chrono::Local;
pub fn init_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = backtrace::Backtrace::new();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        
        // 1. レポートの作成
        let report = format!(
            "CRITICAL PANIC\n\
             Timestamp: {}\n\
             Thread: {}\n\
             Location: {:?}\n\
             Payload: {:?}\n\
             Stack Trace:\n{:?}",
            chrono::Local::now().to_rfc3339(),
            thread_name,
            info.location(),
            info.payload(),
            backtrace
        );
        
        // 2. エラーログへの同期書き込み
        // 非同期ロガーが既に死んでいる可能性を考慮し、std::eprintln!も併用
        eprintln!("{}", report);
        
        // 3. クラッシュダンプファイルの作成
        let dump_path = std::env::temp_dir().join(format!("lightning_filer_crash_{}.txt", chrono::Local::now().format("%Y%m%d_%H%M%S")));
        let _ = std::fs::write(&dump_path, &report);
        
        // 4. ユーザーへの緊急ダイアログ (Win32 API MessageBoxW)
        // GUIループが死んでいても表示できるよう、unsafeでWindows APIを直接叩く
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        use windows::core::HSTRING;
        
        let msg = format!("予期せぬエラーが発生しました。\nログファイル: {}\n\n{}", dump_path.display(), info);
        unsafe {
            MessageBoxW(None, &HSTRING::from(msg), &HSTRING::from("Fatal Error"), MB_ICONERROR | MB_OK);
        }
        
        // 5. 強制終了
        std::process::exit(1);
    }));
}
```

### 2.3 デッドロック検出
開発ビルド (`debug_assertions`) 時のみ、`parking_lot::DeadlockDetector` を別スレッドで起動する。
10秒ごとにロックの依存関係グラフをチェックし、サイクル（デッドロック）を検知したら標準エラー出力に警告を吐く。

```rust
// Cargo.toml の parking_lot 行に
parking_lot = { version = "0.12", features = ["deadlock_detection"] } 
```
```rust
// 起動時に
if cfg!(debug_assertions) {
    std::thread::spawn(|| {
        parking_lot::deadlock::Detector::new()
            .check_interval(std::time::Duration::from_secs(10))
            .run();
    });
}
```
---

## 3. プロセス・スレッドモデル詳細

### 3.1 プロセス構成と生存管理 (Zombie Prevention)

プロセスの親子関係と、異常終了・正常終了双方に対応したライフサイクル管理。

*   **Main Process (64bit)**:
    *   アプリケーションの主体。
    *   **終了処理 (Drop Trait)**:
        *   `AppState` または `BridgeClient` の `Drop` 実装において、Bridgeプロセスへの `Shutdown` コマンド送信を行う。
        *   これを実装しないと、一時ファイルや共有メモリハンドルがリークする可能性がある。

*   **Susie Bridge (32bit)**:
    *   **Job Object (異常終了対策)**:
        *   Main起動時に `CreateJobObjectW` を作成。
        *   `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` の `LimitFlags` に `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` を設定。
        *   Bridge起動直後に `AssignProcessToJobObject` で登録。
        *   効果: Mainがタスクマネージャ等で強制終了（Kill）された場合、OSが即座にBridgeを道連れにする。
    *   **グレースフルシャットダウン (正常終了対策)**:
        *   Mainからの `BridgeCommand::Shutdown` を受信した場合の処理フロー：
            1.  ロード中のSusieプラグインDLLを全て `FreeLibrary` する（ロック解除）。
            2.  作成した一時ファイル（書庫展開用など）を削除する。
            3.  共有メモリのハンドルを全てCloseする。
            4.  プロセスを終了コード0で終了する。
    *   **ウォッチドッグ**:
        *   Mainプロセスとのパイプ接続が切断された場合（`BrokenPipe`）、Job Objectが機能しなかった場合の保険として、自発的にプロセスを終了するロジックを組み込む。

### 3.2 スレッドプールと役割分担
UIのレスポンスを最優先するため、スレッドごとの役割と制約を厳格に定義する。

| スレッド名 | 実装 | スレッド数 | 役割 | 禁止事項 (Strict Rules) |
| :--- | :--- | :--- | :--- | :--- |
| **UI (Main)** | `winit` | 1 (固定) | 描画コマンド発行、入力受付、ウィンドウメッセージ処理 | ・File I/O<br>・`Mutex.lock()` (TryLockのみ可)<br>・1msを超える計算<br>・`tokio::block_on` |
| **Async IO** | `tokio` | コア数 | DBクエリ、IPC通信、ファイル監視、サムネイル読み書き | ・CPUバウンドな計算（画像デコード等） |
| **Compute** | `rayon` | コア数 | 画像デコード、ハッシュ計算、文字コード判別 | ・I/O待ち（スレッドを占有してしまうため） |
| **Watchdog** | `std::thread` | 1 | Bridgeプロセスの死活監視、メモリ使用量監視 | ・パニック（アプリ全体を落とすため） |

---

## 4. IPC通信と共有メモリ転送の実装詳細

Main(64bit)とBridge(32bit)の間で、数MB～数百MBの画像データをゼロコピーで受け渡すための、本プロジェクトの核心となる技術仕様。

### 4.1 通信プロトコル定義 (`ipc_proto`)
`bincode` (Little Endian固定) を使用し、高速かつ型安全な通信を行う。

```rust
// Main -> Bridge (Named Pipe)
#[derive(Serialize, Deserialize, Debug)]
pub enum BridgeCommand {
    LoadPlugin { path: String },
    // Susie API: GetPicture
    GetPicture { 
        file_path: String, // Susie用にShift-JIS変換が必要な場合があるため、生パスではなく変換後を送る
        offset: usize,
        total_size: usize 
    },
    Ping,
    Shutdown,
}

// Bridge -> Main (Named Pipe)
#[derive(Serialize, Deserialize, Debug)]

// フィールド名を aligned_stride に変更
pub enum BridgeResponse {
    ImageReady {
        shmem_handle: String,
        width: u32,
        height: u32,
        aligned_stride: u32,    // ←名称変更
        format: PixelFormat,
        size: usize
    },
    …
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ErrorCode {
    PluginNotFound,
    FileAccessDenied,
    DecodeFailed,
    MemoryAllocationFailed,
}
```
### 4.2 通信チャネル設計

管理者権限やサービス経由での起動など、異なる権限レベル間でも通信を確立できるよう、Windowsのセキュリティモデル（DACL）を明示的に制御する。

*   **セキュリティ属性 (Security Attributes)**:
    *   `CreateNamedPipeW` および `CreateFileMappingW` の `lpSecurityAttributes` 引数には、`NULL`（デフォルト）を使用しない。
    *   **SDDL (Security Descriptor Definition Language)** を使用して、以下の権限を持つセキュリティ記述子を作成・適用する。
        *   `LocalSystem`: Full Access
        *   `Administrators`: Full Access
        *   `Authenticated Users`: Read/Write
    *   実装には `windows-rs` の `ConvertStringSecurityDescriptorToSecurityDescriptorW` を使用するか、ヘルパークレートを用いて安全に構築する。

*   **Control Channel (Named Pipe)**:
    *   **方式**: 双方向、非同期 (`tokio::net::windows::named_pipe`).
    *   **モード**: `PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE` (パケット単位での読み書きを保証)。
    *   **シリアライズ**: `bincode` を使用。
        *   **エンディアン**: 必ず `Little Endian` に固定する（x86/x64間通信のため通常問題ないが、仕様として固定する）。
        *   **バッファリング**: パイプのバッファサイズは `64KB` 以上を確保し、コマンド詰まりを防ぐ。

*   **Data Channel (Shared Memory)**:
    *   **方式**: Windows API (`FileMapping`) のRawアクセス。
    *   **名前空間**: `Local\` プレフィックスを使用し、セッションローカルな名前空間に限定する（多重ユーザー環境での衝突防止）。

### 4.3 ゼロコピー転送フロー (The Zero-Copy Pipeline)

Main(64bit)とBridge(32bit)間で、WGPUのハードウェア制約（256バイトアライメント）を遵守しつつ、メモリコピーを最小限に抑える転送プロトコル。

1.  **Mainプロセス**:
    *   `BridgeCommand::GetPicture` を送信。

2.  **Bridgeプロセス (Padding & Mapping)**:
    *   SusieプラグインAPI (`GetPicture`) を実行し、ローカルヒープ上にDIB (Device Independent Bitmap) を展開する。
    *   **アライメント計算 (重要)**:
        *   画像の `width` と `bpp` (Bytes Per Pixel) から、本来のストライド（1行あたりのバイト数）を計算する。
        *   WGPUの要求仕様 `COPY_BYTES_PER_ROW_ALIGNMENT` (256バイト) に合わせ、パディングを含めた `aligned_stride` を計算する。
        *   計算式: `aligned_stride = (original_stride + 255) & !255`
    *   **共有メモリ作成**:
        *   サイズ: `aligned_stride * height` バイト。
        *   API: `CreateFileMappingW` (PageFile利用)。名前はUUIDでランダム化 (`Local\LF_IMG_{UUID}`)。
        *   セキュリティ属性: 適切なDACLを設定（詳細は本節4.2.1を参照。）。
    *   **データ書き込み**:
        *   `MapViewOfFile` でマップする。
        *   **行ごとのコピー**: 元画像のデータを1行ずつコピーし、行末の余剰部分（パディング）は未初期化またはゼロ埋めとする。これにより、GPUが直接読み取れるメモリレイアウトを形成する。
    *   **完了通知**:
        *   `BridgeResponse::ImageReady` を返信。ペイロードには `shmem_handle`, `width`, `height`, `format` に加え、**`aligned_stride`** を必ず含める。
        *   ハンドルはまだCloseしない。

3.  **Mainプロセス (Direct Upload)**:
    *   `ImageReady` を受信。
    *   `OpenFileMappingW` -> `MapViewOfFile` で読み取り専用 (`FILE_MAP_READ`) としてマップ。
    *   **WGPUへの転送**:
        *   `wgpu::Queue::write_texture` を呼び出す。
        *   `data`: マップした共有メモリのポインタ (`&[u8]`) を直接渡す。
        *   `wgpu::ImageDataLayout`:
            *   `offset`: 0
            *   `bytes_per_row`: Bridgeから受け取った **`aligned_stride`** を指定する（ここが不一致だとパニックする）。
            *   `rows_per_image`: `height`
    *   転送完了後、`UnmapViewOfFile` -> `CloseHandle` でリソースを解放。
    *   Bridgeへ「受信完了」シグナル（または次のコマンド）を送る。

4.  **Bridgeプロセス**:
    *   Mainからの完了通知を受け取ったら、共有メモリのHandleをCloseして破棄。


### 4.4 エラーハンドリング (IPC)
プロセス間通信における異常系を網羅する。

*   **タイムアウト (Timeout)**:
    *   `tokio::time::timeout` を使用。Bridgeからの応答が設定時間（デフォルト3秒）以内にない場合、Susieプラグインが無限ループまたはデッドロックしていると判断する。
    *   **処置**: Bridgeプロセスを `TerminateProcess` で強制終了し、新しいBridgeプロセスを起動する。ユーザーには「プラグイン応答なし：再起動しました」と通知。
*   **パイプ切断 (Broken Pipe)**:
    *   Bridgeプロセスがクラッシュした場合、Pipeの読み書きで `BrokenPipe` エラーが発生する。
    *   **処置**: 即座に再接続ロジックへ移行。リトライ回数制限（例: 3回）を設け、連続して失敗する場合は該当プラグインをブラックリストに入れる。
*   **不正なデータ (Corrupted Data)**:
    *   共有メモリから読み出したデータのヘッダ情報（幅、高さ）が異常な値（0や極端な巨大値）の場合。
    *   **処置**: パニックせずに `AppError::InvalidImageData` を返し、ダミー画像（Broken Imageアイコン）を表示する。
*   **共有メモリ作成失敗**:
    *   Bridge側でメモリ不足により `CreateFileMapping` が失敗した場合。
    *   **処置**: `BridgeResponse::Error { code: MemoryAllocationFailed }` を返し、Main側はL2キャッシュ（RAM）をパージしてからリトライを試みる。


## 5. ファイルシステムと文字コードの完全掌握

Windowsのファイルシステム（NTFS）は、Unicode（UTF-16）をベースとしているが、必ずしも有効なUnicode文字列であるとは限らない（不正なサロゲートペアを含む可能性がある）。一方、Rustの `String` や SQLiteは完全なUTF-8を要求する。この「インピーダンスミスマッチ」を解消し、**「どんなファイル名でも絶対に開ける、落ちない」**仕組みを構築する。

### 5.1 パス構造体設計 (`UniversalPath`)
システム全体でパスを回すための統一構造体。`std::path::PathBuf` の単なるラッパーではなく、DB保存用とUI表示用のデータを内包する。


Windowsのパス長制限（MAX_PATH = 260文字）を突破し、かつRust/SQLiteとの親和性を保つための厳格なパス管理。

*   **構造体定義 (`UniversalPath`)**:
    *   `raw: PathBuf`: ファイルシステム操作用。
    *   `display: String`: UI表示用（Lossy UTF-8）。
    *   `id: u64`: DB検索用ハッシュ。

*   **正規化ロジック (Normalization Logic)**:
    *   `UniversalPath::new(path)` の初期化時に以下の処理を必須とする。
    1.  **絶対パス化**: 相対パスであれば、カレントディレクトリを結合して絶対パスにする。
    2.  **UNCプレフィックス付与**:
        *   パスが `\\?\` で始まっていない場合、これを付与する（例: `C:\Long\Path...` -> `\\?\C:\Long\Path...`）。
        *   これにより、Win32 APIの260文字制限を無効化し、約32,767文字まで扱えるようにする。
        *   `dunce` クレートの `canonicalize` はUNCを解除する場合があるため、自前で制御するか、`verbatim` オプションを使用する。
    3.  **正規化**: `..` や `.` を解決し、パス文字列を一意にする。

*   **DB保存**:
    *   SQLiteの `path_blob` カラムには、このUNCプレフィックス付きの `PathBuf` (OsString) の内部バイト列をそのまま保存する。
    *   これにより、深い階層にあるファイルも問題なくアクセス・復元可能とする。


### 5.2 データベーススキーマ (SQLite)
SQLiteはUTF-8しかインデックスできないため、以下のスキーマで整合性を保つ。`rusqlite` を使用してアクセスする。

```sql
-- Files Table: ファイルシステムの状態をミラーリング
CREATE TABLE files (
    file_id INTEGER PRIMARY KEY,
    
    -- 高速検索・同定用 (UniversalPath.id)
    path_hash INTEGER NOT NULL UNIQUE, 
    
    -- 検索・表示用 (UTF-8, Lossy)
    -- インデックスを張り、LIKE検索やFTS(全文検索)に使用
    path_display TEXT NOT NULL,
    
    -- 復元用 (BLOB)
    -- WindowsのWCHAR配列(u16)をバイト列としてそのまま保存
    -- アプリ起動時やファイルアクセス時は、ここからPathBufを復元する
    -- これにより、UTF-8変換不可能なパスも完全に復元できる
    path_blob BLOB NOT NULL,
    
    parent_hash INTEGER NOT NULL, -- 親フォルダのハッシュ
    size INTEGER,
    modified_at INTEGER,
    
    -- メタデータキャッシュ（JSON形式で拡張性を持たせる）
    metadata TEXT 
);

-- インデックス設計
CREATE INDEX idx_files_parent ON files(parent_hash);
CREATE INDEX idx_files_path_display ON files(path_display);
```

### 5.3 レガシー書庫のエンコーディング解決フロー
Zipクレート (`zip`) はデフォルトでUTF-8を期待するが、CP932（Shift_JIS）等の場合は文字化けする。これを自動解決する。

1.  **Raw Bytes取得**:
    *   書庫ヘッダからファイル名の「生のバイト列 (`Vec<u8>`)」を取得する（`zip::read::ZipFile::name_raw` 相当）。
2.  **UTF-8検証**:
    *   `std::str::from_utf8` で検証。成功ならそのまま採用。
3.  **自動判別 (Heuristic)**:
    *   失敗した場合、`chardetng` クレートにバイト列を食わせて、確率の高いエンコーディングを推測する。
    *   日本語環境 (`LOCALE_SYSTEM_DEFAULT`) の場合、Shift_JIS (CP932) の優先度を上げるロジックを挟む。
4.  **強制変換 (Decode)**:
    *   推測されたエンコーディングで `encoding_rs` を使いデコードする。
5.  **最終防衛ライン (Fallback)**:
    *   それでもデコード不能なバイト列が含まれる場合（破損、特殊な制御文字）、`%XX` 形式のURLエスケープシーケンスとしてファイル名に含め、ユニーク性を保つ。
    *   **絶対に「解凍不能」にしてはならない。**

### 5.4 文字コード判別クレートの統一  

    *   ファイルシステム抽象化層 (`app_fs`) での文字コード判別に使うのは `chardetng` に統一。  
    *   仕様書中の「chardet」表記をすべて「chardetng」へ修正。

```rust
// 例: ZIP 展開時
let detector = chardetng::EncodingDetector::new();
let enc = detector.detect(Some(&raw_bytes), true);
```
---

## 6. メモリ管理とOOM (Out of Memory) 対策

RustはGCを持たないため、メモリ管理はRAIIと明示的なキャッシュ制御に依存する。数百万枚の画像を扱う本アプリでは、OSによる強制終了（OOM Killer）を防ぐため、物理メモリ使用量を厳密に制御する。

### 6.1 リソースマネージャ (`ResourceManager`)
VRAM（GPUメモリ）とRAM（メインメモリ）の使用量を追跡し、上限を超えたら破棄する。

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use lru::LruCache;

pub struct ResourceManager {
    // VRAMキャッシュ: 表示中の画像 (wgpu::Texture)
    // 重み付きLRU: キー=画像ID, 値=テクスチャ, 重み=バイトサイズ
    // 上限目安: VRAMの50% (設定可)
    textures: Arc<RwLock<LruCache<u64, Arc<wgpu::Texture>>>>,

    // RAMキャッシュ: デコード済みだがVRAM未転送、または先読み分
    // 上限目安: 物理メモリの20%
    bitmaps: Arc<RwLock<LruCache<u64, Arc<image::RgbaImage>>>>,
    
    // 現在ロード中のタスク管理（重複ロード防止）
    loading_tasks: DashMap<u64, tokio::sync::broadcast::Sender<Arc<wgpu::Texture>>>,
}
```

### 6.2 メモリ監視と緊急パージ (Watchdog Strategy)
*   **Watchdogスレッド**:
    *   1秒ごとに `windows::Win32::System::SystemInformation::GlobalMemoryStatusEx` を呼び出し、システム全体のメモリ使用率を監視する。
    *   **Yellow Zone (使用率 80%)**:
        *   L2キャッシュ（RAM上のBitmap）の半分をLRU順に破棄。
        *   先読み（Prefetch）を一時停止。
    *   **Red Zone (使用率 90%)**:
        *   L1キャッシュ（VRAM）を含め、**現在表示中（Active Viewport）以外の全てのリソース**を強制的に `Drop` する。
        *   `wgpu::Device::poll(Maintain::Wait)` を呼び、GPU側のメモリ解放を確定させる。

### 6.3 巨大画像対策 (Tiling Strategy)
*   **問題**: `wgpu` の `Limits::max_texture_dimension_2d` (通常 8192px または 16384px) を超える画像は作成できず、パニックする。
*   **実装**:
    1.  デコード時に画像サイズをチェック。
    2.  **縮小ロード**: ビューアでの全体表示用に、長辺を制限内（例: 4096px）に収めた縮小版を作成してテクスチャ化する（Lanczos3使用）。
    3.  **タイル分割**: 拡大表示（等倍・ルーペ）が必要になった瞬間のみ、元画像を `N x M` のタイル（例: 1024x1024）に分割し、**現在表示されている領域（View Area）に必要なタイルだけ**をデコード・転送する。

---

## 7. エラーハンドリングとリカバリ体系

エラーを型システムで厳密に分類し、対応を強制する。`anyhow` はアプリの最上位層でのみ使用し、ライブラリ層では `thiserror` で構造化する。

### 7.1 アプリケーションエラー定義 (`AppError`)

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    // リカバリ可能: ユーザーに通知（Toast/Log）して継続
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Plugin error: {0}")]
    Plugin(String),

    // リカバリ対象: 内部状態で復旧を試みる（ユーザーには一瞬の暗転のみ）
    #[error("GPU Device Lost")]
    GpuLost, 

    // 致命的: アプリ終了またはエラー画面への遷移
    #[error("Database corruption: {0}")]
    DbCorruption(#[from] rusqlite::Error),
    
    #[error("System resource exhaustion")]
    SystemResource(String),
}
```

### 7.2 GPUデバイスロスト対策 (The Robust Renderer)
Windows環境では、ドライバ更新、TDR（Timeout Detection and Recovery）、スリープ復帰などで `wgpu::Device` が無効になることが頻繁にある。これをハンドリングしないとアプリは落ちる。

*   **検知**:
    *   `wgpu::Queue::submit` や `Surface::get_current_texture` が `wgpu::SurfaceError::Lost` や `DeviceLost` を返した時。
*   **復旧フロー (Recovery Routine)**:
    1.  **Pause**: レンダーループ（UIスレッド）を一時停止。
    2.  **Drop**: 既存の `Texture`, `BindGroup`, `Pipeline`, `Surface` を全て破棄（Drop）。`ResourceManager` 内の `Arc<Texture>` も全て無効化する。
    3.  **Re-create**: `wgpu::Instance::request_adapter` -> `request_device` でデバイスを再生成。
    4.  **Re-load**: 現在のアクティブなViewが必要としている画像のみ、ファイルから再ロード・再転送を行う。
    5.  **Resume**: レンダーループ再開。

---

## 8. 実装ロードマップ（Phase 1: Core Foundation）

本仕様書に基づき、以下の順序で実装を開始する。各ステップで「単体テスト」を通すことを条件とする。

1.  **Logging & Panic Hook**:
    *   `app_log` クレートの実装。
    *   意図的に `panic!` させ、ログファイルとクラッシュダンプが生成されることを確認。
2.  **IPC Skeleton**:
    *   `susie_host` (32bit) と `app_main` (64bit) のビルド環境構築。
    *   パイプ通信の疎通テスト。
    *   ダミーデータ（10MB程度のバイト列）を用いた共有メモリ転送のベンチマークテスト。
3.  **Database & VFS**:
    *   SQLiteスキーマの適用。
    *   `UniversalPath` の実装。
    *   Shift-JIS等の「ダメ文字」を含むファイルパスの保存・復元テスト。
4.  **Window & GPU Core**:
    *   `winit` イベントループの構築。
    *   `wgpu` 初期化と、デバイスロスト発生時の復旧ロジックの実装（シミュレーション）。

以上が、`LightningFiler` の堅牢性を担保するためのシステムアーキテクチャ詳細仕様書の全容である。
実装者はこの設計を逸脱せず、特にスレッドモデルとメモリ管理のルールを遵守すること。