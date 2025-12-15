
# Doc 5: 拡張・プラグイン仕様書 (Ver 1.0)

本ドキュメントは、`LightningFiler` の機能を拡張するためのインターフェース仕様を定義する。
実装者は、異なるメモリ空間・言語間でのデータ受け渡しにおける「安全性」と「パフォーマンス」のトレードオフを正確に理解し、記述されたメモリレイアウトとエラー処理を厳守すること。

## 1. プラグインアーキテクチャ概要

3つの異なるレイヤーで拡張性を提供する。

| 種別 | 技術スタック | 用途 | 実行空間 | 安全性 |
| :--- | :--- | :--- | :--- | :--- |
| **Native Plugin** | Rust (`cdylib`) | 高速画像処理、AI解析、独自フォーマット | Main Process (DLL) | **Unsafe** (クラッシュ＝アプリ死) |
| **Legacy Bridge** | Susie API (C/C++) | 古い書庫/画像形式のサポート | Sub Process (IPC) | **Safe** (隔離) |
| **User Script** | Lua (mlua) | バッチ処理、自動タグ付け、UI操作 | Main Process (VM) | **Safe** (サンドボックス) |

---

## 2. Native Plugin API (Rust C-ABI)

最高速で動作する必要がある機能（AI超解像、特殊な画像フィルタ、メタデータ解析）のためのインターフェース。
RustのABIは不安定であるため、**厳密なC-ABI (`extern "C"`)** を定義して連携する。

### 2.1 エントリーポイントとライフサイクル
プラグインDLLは以下のシンボルをエクスポートしなければならない。

```rust
#[repr(C)]
pub struct PluginInfo {
    pub api_version: u32,      // APIバージョン (互換性チェック用)
    pub name: *const c_char,   // プラグイン名 (UTF-8, Null-terminated)
    pub version: *const c_char,// プラグインバージョン
    pub kind: PluginKind,      // ImageLoader | ImageFilter | MetadataParser
}

// 必須エクスポート関数
#[no_mangle]
pub extern "C" fn lf_plugin_init() -> *mut PluginInfo;

#[no_mangle]
pub extern "C" fn lf_plugin_cleanup();
```

### 2.2 画像データ受け渡し (Zero-Copy FFI)
`wgpu` や `image` クレートの構造体をそのまま渡すことはできない。Rawポインタとレイアウト情報でやり取りする。

```rust
#[repr(C)]
pub struct ImageBuffer {
    pub ptr: *mut u8,       // データ先頭ポインタ
    pub len: usize,         // データ長
    pub width: u32,
    pub height: u32,
    pub stride: u32,        // 1行のバイト数 (パディング含む)
    pub format: PixelFormat,// Rgba8, Bgra8, Gray8...
}

// フィルタプラグインの例
#[no_mangle]
pub extern "C" fn lf_apply_filter(
    src: *const ImageBuffer, 
    dst: *mut ImageBuffer, 
    params: *const c_char // JSONパラメータ
) -> PluginResult;
```

### 2.3 UIフック (Overlay Injection)
プラグインが直接描画することは禁止する（`wgpu` コンテキストの競合を防ぐため）。
代わりに、**「描画命令リスト（Primitive List）」**を返す方式を採用する。

*   **ユースケース**: AI顔認識プラグインが、検出した顔に「枠」を表示する。
*   **データ構造**:
    ```rust
    #[repr(C)]
    pub struct OverlayItem {
        pub kind: OverlayKind, // Rect, Text, Circle
        pub x: f32, pub y: f32, pub w: f32, pub h: f32,
        pub color: u32,        // 0xAARRGGBB
        pub text: *const c_char,
    }
    ```

### 2.4 エラーハンドリングと安全性 (AI実装時の注意)
*   **`catch_unwind` 必須**: プラグイン側でパニックが発生した場合、FFI境界を越えてスタック巻き戻しが起きると未定義動作（UB）になる。必ず `std::panic::catch_unwind` で捕捉し、エラーコードを返すこと。
*   **メモリ確保/解放の責任**: 「プラグインが確保したメモリは、プラグインが解放する」原則を守る。`lf_free_buffer` のような関数をエクスポートさせる。

---

## 3. Legacy Bridge (Susie Plugin) 詳細仕様

Doc 1 で定義したIPC通信の、具体的なAPIマッピング。

### 3.1 Susie API マッピング
Susie APIはWindows固有かつ古いため、文字コード変換とポインタ操作に細心の注意が必要。

| Susie API | Bridge Command | 処理内容 |
| :--- | :--- | :--- |
| `GetPluginInfo` | `LoadPlugin` | DLLロード、対応拡張子(`*.jpg`等)の取得。 |
| `IsSupported` | (Internal) | ファイルヘッダ(2KB)を読み込み、対応形式か判定。 |
| `GetPicture` | `GetPicture` | 画像を展開。**256byteアライメント**を適用して共有メモリへ書き込む。 |
| `GetArchiveInfo` | `GetArchiveList` | 書庫内のファイル一覧を取得。**Shift_JIS -> UTF-8変換**を行う。 |
| `GetFile` | `ExtractFile` | 書庫内の特定ファイルをメモリまたは一時ファイルに展開。 |

### 3.2 エラーハンドリング (Bridge Process)
*   **SPIのバグ対策**: 古いSPIは、不正なファイルを与えるとAccess Violationで落ちることがある。
    *   Bridgeプロセスは、SPI呼び出しを `SEH` (Structured Exception Handling) または `vectored_exception_handler` で囲み、クラッシュを検知したらエラーレスポンスを返して**自発的に再起動**する。
*   **文字化け対策**: SPIが返すファイル名はShift_JIS (CP932) であることがほとんどだが、稀にEUC-JPやUTF-8を返すものがある。`chardetng` で判定してからUTF-8化する。

---

## 4. User Scripting (Lua)

ユーザーが「特定の条件でファイルを移動」「キー操作でタグ付け」などを自動化するためのスクリプト環境。
高速かつ軽量な **LuaJIT (mlua)** を採用する。

### 4.1 公開API (Lua Bindings)
Doc 3 で定義したコマンドIDをスクリプトから呼び出せるようにする。

```lua
-- 例: 現在の画像の評価が3以上なら、"Good"フォルダに移動して次の画像へ
local rating = app.meta.get_rating()
if rating >= 3 then
    app.file.move_to("C:\\Images\\Good")
    app.view.next_item()
end
```

*   **`app.nav.*`**: 移動系コマンド。
*   **`app.view.*`**: 表示系コマンド。
*   **`app.file.*`**: ファイル操作。
*   **`app.meta.*`**: メタデータ操作。
*   **`app.current_file`**: 現在選択中/表示中のファイル情報（パス、サイズ、Exif）。

### 4.2 セキュリティと制限
*   **サンドボックス**:
    *   `io` (ファイル入出力), `os.execute` (コマンド実行) などの危険な標準ライブラリは**デフォルトで無効化**する。
    *   ファイル操作は必ず `app.file.*` 経由で行わせることで、アプリ側の管理下（Undo履歴、ログ記録）に置く。
*   **タイムアウト**:
    *   スクリプトの実行時間が 100ms を超えた場合、強制停止させる（UIフリーズ防止）。

---

## 5. AI & Metadata Module (Built-in Plugin)

「AI Meta Viewer」の機能を移植・統合するための仕様。これは「Native Plugin」の一種として実装するが、コア機能として同梱する。

### 5.1 解析パイプライン
1.  **Trigger**: 画像ロード完了時、またはユーザーが「情報パネル」を開いた時。
2.  **Detection**:
    *   **Stealth PNG**: RGB/AlphaチャンネルのLSBを高速スキャン（SIMD最適化）。
    *   **Exif UserComment**: `kamadak-exif` でパース。
    *   **tEXt / iTXt**: PNGチャンク解析。
3.  **Normalization**:
    *   各ツール（Stable Diffusion, NovelAI, Midjourney）の独自フォーマットを、統一構造体 `AiMetadata` に変換する。
    *   プロンプト、ネガティブプロンプト、モデルハッシュ、シード値、ステップ数などを抽出。

### 5.2 データ構造 (`app_core::metadata`)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct AiMetadata {
    pub tool: String, // "Stable Diffusion WebUI", "ComfyUI", etc.
    pub positive_prompt: String,
    pub negative_prompt: String,
    pub params: HashMap<String, String>, // その他パラメータ (Steps, Sampler, Seed...)
    pub raw_source: String, // 解析元の生テキスト
}
```

---

## 6. 実装ロードマップ（Phase 3: Extension）

1.  **Plugin Core**:
    *   `PluginInfo`, `ImageBuffer` 等のFFI用構造体定義。
    *   DLLの動的ロード (`libloading`) とバージョンチェックの実装。
2.  **Susie Bridge**:
    *   `susie_host` プロセスでのSPIロード実装。
    *   `GetPicture` のアライメント調整と共有メモリ書き込み。
3.  **Lua Engine**:
    *   `mlua` の組み込み。
    *   `app.*` APIのバインディング実装。
4.  **AI Module**:
    *   `AI Meta Viewer` のロジック（JS）をRustに移植。
    *   `rayon` を使った並列Stealth PNGデコード。

---

### AIコーディング時の注意点 (Prompting Guide)

*   **FFIの安全性**:
    *   「`unsafe` ブロック内では、ポインタがnullでないこと、アライメントが正しいこと、メモリ範囲外アクセスしないことを確認するコードを必ず挿入せよ」と指示する。
*   **パニック境界**:
    *   「プラグインから呼ばれるコールバック関数、およびプラグインを呼ぶ箇所はすべて `catch_unwind` で保護せよ」と指示する。
*   **文字コード**:
    *   「Susieプラグインにパスを渡す際は、必ずShift_JIS (CP932) に変換可能かチェックし、不可能ならエラーにするか、短いファイル名 (8.3形式) を取得して渡すロジックを入れろ」と指示する。
