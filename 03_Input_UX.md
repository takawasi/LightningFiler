# Doc 3: 入力・操作カスタマイズ仕様書 (Part 1: Navigation - Revised 2)

## 2.1 ナビゲーション (Navigation) `nav.*`

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
