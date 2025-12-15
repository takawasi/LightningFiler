# Doc 3: 入力・操作カスタマイズ仕様書 (Part 1: Navigation - Revised 2)

## 3.1 ナビゲーション (Navigation) `nav.*`

### A. カーソル・フォーカス移動 (Grid / List Context)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `nav.move_up` | 上へ移動 | `amount`(1), `select`(false) | -- |
| `nav.move_down` | 下へ移動 | `amount`(1), `select`(false) | -- |
| `nav.move_left` | 左へ移動 | `amount`(1), `select`(false), `wrap`(false) | `wrap=true`: 行頭で前の行の末尾へ。 |
| `nav.move_right` | 右へ移動 | `amount`(1), `select`(false), `wrap`(false) | `wrap=true`: 行末で次の行の先頭へ。 |
| `nav.page_up` | ページアップ | `amount`(1), `select`(false) | 1画面分上へ。 |
| `nav.page_down` | ページダウン | `amount`(1), `select`(false) | 1画面分下へ。 |
| `nav.home` | 先頭へ | `select`(false) | フォルダ内の最初のファイルへ。 |
| `nav.end` | 末尾へ | `select`(false) | フォルダ内の最後のファイルへ。 |

### B. 論理アイテム移動 (Viewer / Browser Context)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `nav.next_item` | 次のファイル | `amount`(1), `wrap`(false), `cross_folder`(bool:false) | `cross_folder=true`: フォルダ末尾で自動的に次のフォルダの先頭へ移動。 |
| `nav.prev_item` | 前のファイル | `amount`(1), `wrap`(false), `cross_folder`(bool:false) | `cross_folder=true`: フォルダ先頭で前のフォルダの末尾へ移動。 |
| `nav.next_page` | 次のページ | `amount`(1) | Viewer: 書庫内/PDF内の次ページ。 |
| `nav.prev_page` | 前のページ | `amount`(1) | Viewer: 書庫内/PDF内の前ページ。 |

### C. 階層・フォルダ間移動 (Hierarchy)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `nav.enter` | 入る/表示 | **`threshold`(int:5)** | フォルダ/書庫を選択時の挙動。<br>中身のファイル数が `threshold` **以下**なら**Viewerモード**で開き、それより多ければ**Browserモード**で中に入る。<br>※初期値は `5` を推奨（見開き2枚＋α程度ならビューアで見る方が速いため）。 |
| `nav.parent` | 親フォルダへ | -- | -- |
| `nav.next_sibling` | 次のフォルダ | `wrap`(false), `skip_empty`(bool:true) | 隣のフォルダへ移動。**チルト右のデフォルト**。 |
| `nav.prev_sibling` | 前のフォルダ | `wrap`(false), `skip_empty`(bool:true) | 前のフォルダへ移動。**チルト左のデフォルト**。 |
| `nav.root` | ドライブ直下へ | -- | -- |

### D. スクロール (Scroll)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `nav.scroll_y` | 縦スクロール | `amount`(int:null), `unit`(Line/Page/Pixel), `multiplier`(float:1.0) | `amount`がnullの場合、Windows設定値を使用。<br>**マウスホイールのデフォルト**。 |
| `nav.scroll_x` | 横スクロール | `amount`(int:null), `unit`(Line/Page/Pixel) | Viewer拡大時用。Browserでは基本的に使用しない。 |

### E. プレビュー・確認 (Peek / Quick Look)

MacのQuick Look（スペース長押し）のような、カーソル位置のアイテムを一時的に確認する機能です。
機能的には「表示」ですが、探索フローの一部であるため、ここで定義します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.quick_look` | クイックルック | `size`(enum:Fit/Original), `sound`(bool:false) | **Press時**: ポップアップウィンドウ（または全画面）でプレビューを表示。<br>**Release時**: プレビューを閉じる。<br>動画の場合は再生するかも `sound` で制御。 |


## 3.2 ビューア操作 (View) `view.*`

Viewer Context（画像表示中）における、表示・変形・移動・マルチビュー制御を定義します。
Doc 4 で定義された「ビューエリア (View Area)」概念に基づき、操作は原則として**「アクティブなビューエリア」**に対して行われます。

### A. 拡大・縮小 (Zoom & Scale)
Doc 4 の「2.2 初期表示位置」および「4.1 マウス操作」に対応します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.zoom_in` | 拡大 | `step`(float:0.1), `center`(enum:Cursor/Center) | 指定ステップ拡大。`Cursor`はマウス位置中心（Doc 4 4.1準拠）。 |
| `view.zoom_out` | 縮小 | `step`(float:0.1), `center`(enum:Cursor/Center) | 指定ステップ縮小。 |
| `view.zoom_set` | 倍率指定 | `mode`(enum:Original/FitWindow/FitWidth/FitHeight), `scale`(float:1.0), `toggle_origin`(bool:true) | 表示モード指定。`toggle_origin=true`なら、既にそのモードの場合にOriginal（等倍）に戻す（トグル動作）。 |
| `view.zoom_mode_cycle` | モード順次切替 | `modes`(list), `reverse`(bool:false) | [Original, FitWindow, FitWidth] などを順次切替。 |
| `view.lock_zoom` | 倍率ロック | `toggle`(bool:true) | ページ移動しても現在のズーム倍率と位置を維持するかどうか。 |

### B. パン・スクロール (Pan & Scroll)
Doc 4 の「2.3 スナップ」「2.4 オーバースクロール」「3. 移動・スクロールロジック」を制御します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.pan` | パン移動 | `direction`(Up/Down/Left/Right), `amount`(int:10), `unit`(Pixel/Screen) | キーボードによる視点移動。`Screen`指定で画面の○%移動。 |
| `view.pan_to` | 位置ジャンプ | `position`(enum:TopLeft/TopRight/BottomLeft/BottomRight/Center) | 画像の四隅や中央へ瞬時に視点を移動。 |
| `view.scroll_up` | 上スクロール | `amount`(int), `unit`(Pixel/Line) | 画像を上へ移動。端なら何もしない（**抵抗モード**）。 |
| `view.scroll_down` | 下スクロール | `amount`(int), `unit`(Pixel/Line) | 画像を下へ移動。端なら何もしない。 |
| `view.smart_scroll_up` | スマート上 | `overlap`(int:50) | 上へスクロール。端なら**前の画像**へ（**スマートスクロール**）。 |
| `view.smart_scroll_down` | スマート下 | `overlap`(int:50) | 下へスクロール。端なら**次の画像**へ。 |
| `view.scroll_n_type_up` | N字上送り | `overlap`(int:50) | **N字スクロール**順序で戻る。 |
| `view.scroll_n_type_down` | N字下送り | `overlap`(int:50) | **N字スクロール**順序で進む。 |
| `view.toggle_snap` | スナップ切替 | `toggle`(bool:true) | 画像端への吸着機能（Doc 4 2.3）のON/OFF。 |

### C. マルチビュー・比較制御 (Multi-View & Compare)
Doc 4 の「1.1 ビューエリア」「3.3 同期スクロール」を制御します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.split_mode` | 画面分割 | `mode`(enum:Single/Vertical/Horizontal), `toggle`(bool:true) | 画面を分割し、複数のビューエリアを表示する。 |
| `view.next_view_area` | 次のビュー | -- | アクティブなビューエリア（操作対象）を切り替える。 |
| `view.sync_scroll` | 同期スクロール | `mode`(enum:None/Position/Relative), `toggle`(bool:true) | 複数のビューエリアの移動・ズームを同期させる。<br>`Position`: 同じ座標を表示。<br>`Relative`: 現在のズレを維持して同期。 |
| `view.copy_view_state` | 状態コピー | -- | アクティブなビューの状態（ズーム、位置）を、他のビューにコピーする。 |

### D. ビューア内ナビゲーション (Viewer Navigation)
Doc 4 の「1.3 B 下部シークバー」や「4.3 キーボード操作」に対応する移動コマンドです。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.next_item` | 次の画像 | `amount`(int:1), `cross_folder`(bool:false) | 次の画像へ。`amount`指定で10枚飛ばし等が可能。 |
| `view.prev_item` | 前の画像 | `amount`(int:1), `cross_folder`(bool:false) | 前の画像へ。 |
| `view.next_folder` | 次のフォルダ | `skip_empty`(bool:true) | 次のフォルダへ移動し、**先頭**を表示。 |
| `view.prev_folder` | 前のフォルダ | `skip_empty`(bool:true) | 前のフォルダへ移動し、**末尾**を表示。 |
| `view.seek` | シーク | `position`(float:0.0-1.0) | フォルダ内の位置（%）へジャンプ。シークバー操作のキーボード版。 |
| `view.parent` | 親フォルダ | -- | Viewerを終了し、Browserで親フォルダを表示。 |

### E. スライドショー (Slideshow)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.slideshow` | スライドショー | `action`(enum:Start/Stop/Toggle), `order`(enum:Normal/Reverse/Shuffle/Random) | `Shuffle`: 重複なしランダム。<br>`Random`: 完全ランダム。 |
| `view.slideshow_interval` | 間隔変更 | `amount`(float), `relative`(bool:true) | 再生間隔の調整。 |

### F. 表示設定・エフェクト (Display Settings)
Doc 4 の「1.2 背景色」「1.3 オーバーレイUI」などを制御します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `view.rotate` | 回転 | `angle`(int:90) | 相対回転。 |
| `view.flip` | 反転 | `axis`(enum:Horizontal/Vertical) | 反転トグル。 |
| `view.spread_mode` | 見開きモード | `mode`(enum:Single/Spread/Auto), `toggle`(bool:true) | 見開き設定。 |
| `view.toggle_transition` | 効果切替 | `mode`(enum:None/Fade/Slide), `cycle`(bool:true) | 画像切替時のエフェクト設定を変更。 |
| `view.toggle_info` | 情報表示 | `level`(enum:None/Simple/Detail) | オーバーレイ情報の切替。 |
| `view.toggle_fullscreen` | フルスクリーン | -- | 全画面表示。 |
| `view.toggle_chromeless` | 没入モード | -- | ウィンドウ枠ありでUIのみ非表示。 |
| `view.set_background` | 背景色変更 | `color`(enum:Black/Gray/Check/White/Transparent), `cycle`(bool:true) | 背景色を順次切り替え。 |
承知いたしました。
**Doc 3: 入力・操作カスタマイズ仕様書** の続きとして、**「2.3 ファイル操作 (File)」** を定義します。

ここでは、ファイラーとしての基本機能に加え、貴殿が重視されている**「外部アプリ連携」**や**「クリップボードの使い分け（ファイル vs 画像データ）」**を明確に区別して設計します。


## 2.3 ファイル操作 (File) `file.*`

ファイルシステムに対する変更操作、クリップボード操作、および外部アプリケーションとの連携を定義します。
これらの操作は、**Browser Context**（選択中のファイル群に対して実行）および **Viewer Context**（表示中のファイルに対して実行）の両方で有効です。

### A. クリップボード操作 (Clipboard)
「ファイルそのもの」をコピーするか、「画像データ」をコピーするかを明確に区別します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `file.copy` | コピー | -- | 選択ファイルをクリップボードにコピー（エクスプローラーで貼り付け可能）。 |
| `file.cut` | 切り取り | -- | 選択ファイルをクリップボードにカット。 |
| `file.paste` | 貼り付け | -- | クリップボード内のファイルを現在のフォルダに貼り付け。 |
| `file.copy_image` | 画像コピー | -- | **Viewer専用**。表示中の**画像データ（ビットマップ）**をクリップボードにコピー（Photoshop等への貼り付け用）。 |
| `file.copy_path` | パスコピー | `format`(enum:Full/Name/Dir) | ファイルパスをテキストとしてクリップボードにコピー。<br>`Full`: フルパス<br>`Name`: ファイル名のみ<br>`Dir`: 親ディレクトリパス |

### B. ファイルシステム操作 (File System)
物理的なファイルの移動・削除・変更を行います。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `file.delete` | 削除 | `trash`(bool:true), `confirm`(bool:true) | ファイルを削除。<br>`trash=true`: ゴミ箱へ移動。<br>`trash=false`: **完全削除**（復元不可）。<br>`confirm=false`: 確認ダイアログなしで即実行。 |
| `file.rename` | リネーム | `dialog`(bool:false) | リネームモードへ移行。<br>`dialog=true`: 専用ダイアログを表示。<br>`dialog=false`: インライン編集（Browser時）。 |
| `file.create_dir` | フォルダ作成 | -- | 新規フォルダ作成ダイアログを表示。 |
| `file.copy_to` | フォルダへコピー | `target`(path:null), `dialog`(bool:true) | 指定パスへコピー。<br>`target`指定あり＆`dialog=false`なら、確認なしで即コピー（仕分け用）。 |
| `file.move_to` | フォルダへ移動 | `target`(path:null), `dialog`(bool:true) | 指定パスへ移動。<br>移動後は自動的に次のファイルへフォーカス移動。 |

### C. 外部連携・シェル (External / Shell)
OSの機能や他のアプリケーションを呼び出します。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `file.open_explorer` | エクスプローラー | `select`(bool:true) | エクスプローラーで現在のフォルダを開く。<br>`select=true`: 対象ファイルを選択状態で開く。 |
| `file.open_with` | プログラムから開く | -- | Windows標準の「プログラムから開く」ダイアログを表示。 |
| `file.open_external` | 外部アプリ起動 | **`app_id`(string)**, `args`(string:null) | 事前に設定された外部アプリ（ID指定）で開く。<br>`args`がnullの場合、設定されたデフォルト引数を使用。 |
| `file.properties` | プロパティ | -- | Windows標準のファイルプロパティ画面を表示。 |

---

### 補足：外部アプリ連携の仕様

`file.open_external` で使用する `app_id` と引数マクロの定義です。これらは設定ファイル（`settings.json`）で定義され、コマンドからはIDで参照します。

#### 1. 設定例 (`settings.json`)
```json
"external_apps": {
  "photoshop": {
    "path": "C:\\Program Files\\Adobe\\...\\Photoshop.exe",
    "args": "%f"  // ファイルパスを渡す
  },
  "explorer_select": {
    "path": "explorer.exe",
    "args": "/select,%f"
  },
  "google_search": {
    "path": "https://www.google.com/search?q=%n", // ブラウザで検索
    "is_url": true
  }
}
```

#### 2. 引数マクロ (Macros)
外部アプリに渡すコマンドライン引数として、以下の変数が使用可能です。

| マクロ | 展開内容 | 例 |
| :--- | :--- | :--- |
| `%f` | フルパス (Full Path) | `C:\Images\Photo.jpg` |
| `%d` | ディレクトリパス (Directory) | `C:\Images` |
| `%n` | ファイル名 (Name) | `Photo.jpg` |
| `%s` | ファイル名 (Stem / 拡張子なし) | `Photo` |
| `%e` | 拡張子 (Extension) | `jpg` |
| `%p` | 現在のページ番号 (Page Number) | `1` (Viewer時のみ) |

---

承知いたしました。
**Doc 3: 入力・操作カスタマイズ仕様書** の続きとして、**「2.4 メタデータ操作 (Meta)」** を定義します。

ここでは、Picasaの快適な整理フローを再現するための**「クイックタグ」**や、大量の画像を高速に選別するための**「レーティング・ラベル操作」**を詳細に定義します。

---

# Doc 3: 入力・操作カスタマイズ仕様書 (Part 4: Meta)

## 2.4 メタデータ操作 (Meta) `meta.*`

ファイルのメタデータ（評価、タグ、ラベル、コメント）を操作するコマンド群です。
Browser / Viewer / Slideshow の全コンテキストで有効であり、実行時には画面中央にOSD（On Screen Display）で結果を表示します（例: "★5", "Tag: Good [Added]"）。

### A. レーティング・ラベル (Rating & Label)
標準的な5段階評価と、色による分類機能です。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `meta.rate` | 評価設定 | `value`(int:0-5), `toggle`(bool:true) | 指定した評価を設定。<br>`toggle=true`: 既にその値なら `0` (なし) に戻す。<br>例: キー「5」に `value=5` を割り当て。 |
| `meta.rate_step` | 評価増減 | `amount`(int:1), `loop`(bool:false) | 現在の評価を増減させる。<br>`amount=1`: ★を増やす。<br>`amount=-1`: ★を減らす。 |
| `meta.label` | ラベル設定 | `color`(enum:Red/Blue/Green/Yellow/Purple/None), `toggle`(bool:true) | カラーラベルを設定。Mac/Adobe Bridge互換。<br>`toggle=true`: 既にその色なら `None` に戻す。 |

### B. タグ操作 (Tagging)
Picasaの「クイックタグ」を実現するための核心機能です。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `meta.tag_toggle` | タグ切替 | **`name`(string)** | 指定したタグ名のON/OFFをトグルする。<br>**必須**: キー設定で引数 `name` に「家族」「風景」などを指定して登録する。 |
| `meta.tag_add` | タグ追加 | **`name`(string)** | 指定したタグを強制的に追加する（既に付いていても何もしない）。 |
| `meta.tag_remove` | タグ削除 | **`name`(string)** | 指定したタグを強制的に削除する。 |
| `meta.edit_tags` | タグ編集 | -- | タグ入力ダイアログを開く（インクリメンタルサーチ・履歴付き）。 |

### C. 編集・管理 (Edit & Manage)
効率的な整理を支援する補助機能です。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `meta.copy_meta` | メタデータコピー | `target`(enum:Rating/Tags/All) | **「直前の画像（リスト上の1つ前）」**の評価やタグを、現在の画像にコピーする。<br>連写画像の整理時に、1枚目でタグ付けして残りはこれで連打する。 |
| `meta.edit_comment` | コメント編集 | -- | コメント入力欄を開く。 |
| `meta.toggle_mark` | マーク切替 | -- | **一時的なマーク**（アプリ終了時に消える選択状態）をトグルする。<br>「後でまとめて操作したい」時に使用。 |
| `meta.select_marked` | マークを選択 | -- | マークされたファイルを全て「選択状態」にする（一括コピー/削除用）。 |

---

### キー割り当ての推奨例 (Default Keymap Proposal)

この仕様に基づき、デフォルト（または推奨プリセット）として以下のキー割り当てを想定します。

*   **テンキー 1～5**: `meta.rate` (value=1～5)
*   **テンキー 0**: `meta.rate` (value=0 / 解除)
*   **Ctrl + 1～9**: `meta.tag_toggle` (ユーザー定義タグ 1～9)
*   **Space**: `meta.toggle_mark` (Browser時) または `view.smart_scroll` (Viewer時) ※コンテキストで分離
*   **[`] (Backquote)**: `meta.copy_meta` (直前の状態をコピー)

---

承知いたしました。
**Doc 3: 入力・操作カスタマイズ仕様書** の最後となるセクション、**「2.5 アプリケーション操作 (App)」** を定義します。

ここでは、アプリケーションのライフサイクル管理、ウィンドウ制御、そしてLeeyes/NeeViewのような柔軟なドッキングUIを支える**「レイアウト管理」**について定義します。

---


## 2.5 アプリケーション操作 (App) `app.*`

アプリケーション全体の状態、ウィンドウ、設定、レイアウトを制御するコマンド群です。
これらは **Global Context**（常に有効）として扱われます。

### A. アプリケーション制御 (Lifecycle & Settings)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `app.exit` | 終了 | `confirm`(bool:false) | アプリケーションを終了する。 |
| `app.restart` | 再起動 | -- | アプリケーションを再起動する（設定反映時などに使用）。 |
| `app.open_settings` | 設定画面 | `page`(string:null) | 設定ダイアログを開く。`page`指定で特定のタブ（例: "keymap"）を直接開く。 |
| `app.open_manual` | ヘルプ | -- | オンラインマニュアルまたはREADMEを開く。 |
| `app.about` | バージョン情報 | -- | バージョン情報ダイアログを表示。 |
| `app.clear_cache` | キャッシュクリア | `target`(enum:Thumbnail/Image/All) | メモリ/ディスクキャッシュを破棄する。 |

### B. ウィンドウ操作 (Window)

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `app.minimize` | 最小化 | -- | ウィンドウを最小化する。 |
| `app.maximize` | 最大化/復元 | `toggle`(bool:true) | 最大化状態をトグルする。 |
| `app.topmost` | 常に手前に表示 | `toggle`(bool:true) | ウィンドウを最前面に固定する。 |
| `app.new_window` | 新規ウィンドウ | -- | 新しいウィンドウ（インスタンス）を起動する。 |

### C. レイアウト・パネル操作 (Layout & Panels)
ドッキングUIのパネル（フォルダツリー、プレビュー、情報パネル等）の表示制御です。

| コマンドID | 日本語名 | 引数 (型: デフォルト) | 挙動詳細 |
| :--- | :--- | :--- | :--- |
| `app.toggle_panel` | パネル切替 | **`panel_id`(string)** | 指定したパネル（"tree", "info", "preview" 等）の表示/非表示をトグルする。 |
| `app.focus_panel` | パネルフォーカス | **`panel_id`(string)** | 指定したパネルにフォーカスを移動する。 |
| `app.layout_save` | レイアウト保存 | `slot`(int:1) | 現在のパネル配置をスロットに保存する。 |
| `app.layout_load` | レイアウト読込 | `slot`(int:1) | 保存されたパネル配置を復元する。 |
| `app.layout_reset` | レイアウト初期化 | -- | デフォルトのレイアウトに戻す。 |

---