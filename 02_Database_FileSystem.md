
# Doc 2: データベース・ファイルシステム仕様書 (Ver 2.0 - Definitive Edition)

本ドキュメントは、`LightningFiler` のデータ永続化層とファイルシステム操作の実装詳細を定義する。
実装者は、Windowsファイルシステムの特殊性（パス長、エンコーディング、ロック）を理解し、Rustの型システムを用いてこれらを安全に抽象化すること。

## 1. 技術スタックと選定理由 (Refined)

*   **Metadata DB**: **SQLite (`rusqlite`)**
    *   *構成*: `bundled` 機能を使用し、システムDLLに依存せず静的リンクする。
    *   *プール*: `r2d2` または `deadpool` を使用し、リーダー/ライターを分離して並列性を確保する。
*   **KVS Cache**: **RocksDB (`rust-rocksdb`)**
    *   *注意*: Windowsでのビルドは難易度が高いため、`LIBCLANG_PATH` の設定や `vcpkg` の利用を前提としたビルドスクリプトを用意すること。
    *   *代替案*: ビルドトラブルが解決できない場合のみ、Rust製でPureな `Sled` を検討するが、パフォーマンス優先のためRocksDBを第一候補とする。
*   **File System**: **`std::fs` + `windows-rs`**
    *   *必須*: `MAX_PATH` 制限を回避するため、全てのパス操作に `\\?\` プレフィックスを付与するラッパーを実装する。
*   **Encoding**: **`encoding_rs` + `chardetng`**

---

## 2. データベース設計 (SQLite)

### 2.1 接続と並列性 (Connection Pooling)
SQLiteの `WAL` モードを活かすため、単一の接続ではなくコネクションプールを使用する。

```rust
use r2d2_sqlite::SqliteConnectionManager;

pub type DbPool = r2d2::Pool<SqliteConnectionManager>;

pub fn init_pool(path: &Path) -> Result<DbPool> {
    let manager = SqliteConnectionManager::file(path)
        .with_init(|c| {
            // パフォーマンスチューニング (必須)
            c.execute_batch("
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA cache_size = -64000; -- 64MB
                PRAGMA foreign_keys = ON;
                PRAGMA busy_timeout = 5000;
            ")
        });
        
    r2d2::Pool::builder()
        .max_size(10) // リーダー用スレッド数に合わせる
        .build(manager)
        .map_err(Into::into)
}
```

### 2.2 スキーマ定義 (Schema)
提示されたスキーマを採用するが、以下の点を補強する。

*   **`files` テーブル**:
    *   `path_hash`: 衝突時のリカバリ用に、ハッシュだけでなく `path_blob` も比較するロジックをアプリ側に実装すること。
    *   `attributes`: Windowsのファイル属性 (`GetFileAttributesW`) をそのまま格納する。

*   **`search_history` テーブル**:
    *   プライバシー配慮のため、履歴保存のON/OFF設定と、「期間指定削除」機能を実装しやすい構造にする。

---

## 3. キーバリューストア設計 (RocksDB)

### 3.1 Windowsビルドの注意点
AIにコードを書かせる前に、`Cargo.toml` と環境設定を指示する。

```toml
[dependencies]
rocksdb = { version = "0.21", default-features = false, features = ["lz4", "multi-threaded-cf"] }
```
*   **環境変数**: Windows環境では `LLVM_HOME` の設定が必要になる場合があることをドキュメント化する。

### 3.2 キー設計とバイナリレイアウト
キー生成時のアロケーションを避けるため、バイト配列を直接構築する。

```rust
// キー: [Hash(8B)][Width(4B)][Height(4B)] = 16 Bytes
pub fn make_thumb_key(hash: u64, width: u32, height: u32) -> [u8; 16] {
    let mut key = [0u8; 16];
    key[0..8].copy_from_slice(&hash.to_be_bytes()); // Big Endianでソート順を保つ
    key[8..12].copy_from_slice(&width.to_be_bytes());
    key[12..16].copy_from_slice(&height.to_be_bytes());
    key
}
```

---

## 4. 仮想ファイルシステム (VFS) 実装詳細

### 4.1 `UniversalPath` と UNCパス
Windowsのパス長制限（260文字）を突破するための絶対ルール。

*   **正規化ロジック**:
    1.  相対パスは絶対パスに変換。
    2.  `\\?\` プレフィックスがない場合、付与する。
    3.  `\` (バックスラッシュ) を使用する（`/` はWindows APIによっては弾かれる）。

```rust
pub fn to_unc_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with(r"\\?\") {
        path.to_path_buf()
    } else {
        PathBuf::from(format!(r"\\?\{}", path_str))
    }
}
```

### 4.2 アーカイブVFS (`ArchiveFS`)
書庫内のファイルを「あたかもフォルダのように」扱うための透過レイヤー。

*   **パス表現**: `archive://{ArchiveHash}/{InnerPath}` のようなURIスキーム、または `UniversalPath` 内に `inner_path: Option<String>` フィールドを持たせる。
*   **リストキャッシュ**:
    *   Zipのセントラルディレクトリは、一度読んだらSQLiteの `files` テーブル（または専用の `archive_entries` テーブル）にキャッシュし、2回目以降の展開を高速化する。

---

## 5. ファイルシステム監視 (Watcher)

### 5.1 イベントストーム対策 (Debounce)
`notify` クレートからのイベントは、1つのファイル操作で複数回発生する（Create -> Modify -> Modify -> Close）。これを間引く。

```rust
use notify::DebouncedEvent;
use std::sync::mpsc::channel;
use std::time::Duration;

// 100msの遅延を持たせてイベントをまとめる
let (tx, rx) = channel();
let mut watcher = notify::watcher(tx, Duration::from_millis(100))?;
```
*   **注意**: `notify` v5/v6 ではAPIが異なるため、最新の `notify` クレートの仕様（`RecommendedWatcher`）に従うこと。

### 5.2 データベースとの同期
*   **Created**: `files` テーブルにINSERT。サムネイル生成キューに追加。
*   **Deleted**: `files` テーブルからDELETE（外部キー制約でタグなども消える）。RocksDBからサムネイル削除。
*   **Renamed**: `path_blob`, `path_display`, `path_hash` を更新。ハッシュが変わるため、サムネイルは再生成が必要（またはハッシュ変更に対応するロジック）。

---

## 6. エラーハンドリングとエッジケース (Implementation Guide)

### 6.1 ファイルロック (Sharing Violation)
他のアプリ（ウイルス対策ソフト、エクスプローラー）がファイルを掴んでいる場合の対策。

*   **リトライロジック**:
    *   `std::fs::File::open` が `OS Error 32` で失敗した場合、10ms待機して最大5回リトライするラッパー関数 `fs_retry::open` を実装し、必ずそれを使用する。

### 6.2 代替データストリーム (ADS)
*   **Zone.Identifier**:
    *   WebからDLした画像には「セキュリティのブロック」情報が付いている。
    *   読み込みには影響しないが、**ファイル移動・コピー時**にこのストリームも一緒に移動させる必要がある（`std::fs` は通常これを処理するが、低レベルAPIを使う場合は注意）。

### 6.3 破損した画像・書庫
*   **ゼロバイトファイル**:
    *   サイズが0のファイルは、デコーダに渡す前に弾く。
*   **不完全な書庫**:
    *   Zipの末尾が欠けている場合など、パニックせずに「読み込める部分まで読む」か、エラーとしてスキップする。`catch_unwind` で保護する。

---

## 7. 実装ロードマップ（Phase 2: Data & FS）

1.  **DB Core**:
    *   `app_db` クレート作成。
    *   SQLiteスキーマ適用と、`r2d2` プールのセットアップ。
    *   RocksDBのビルド確認と、バイナリ読み書きテスト。
2.  **VFS Core**:
    *   `UniversalPath` の実装と、UNCパス変換の単体テスト（特に日本語・絵文字パス）。
    *   `PhysicalFS` の実装。ディレクトリ列挙のベンチマーク。
3.  **Scanner & Watcher**:
    *   指定フォルダを再帰的にスキャンし、SQLiteにインポートする `Indexer`。
    *   `notify` を用いた監視と、イベント間引きロジックの実装。

以上が、`LightningFiler` のデータ基盤となる詳細仕様書である。
この仕様に従うことで、数百万ファイルの管理に耐えうる堅牢なバックエンドが構築される。