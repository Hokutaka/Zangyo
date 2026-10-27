# Zangyo

Zangyoは、日本語のキーワードで記述したプログラムをx86-64のオブジェクトファイルへコンパイルする、ネタプログラミング言語です。

[Rust製のアセンブラライブラリ Sashimi](https://github.com/Cerune-Lang/Sashimi) の型付き命令・`Program` APIを使用し、Linux向けのELF64とWindows向けのCOFFを生成します。生成した命令・データのセクション内位置を、ソース行と関連付けて出力することもできます。

**開発状況：初期プロトタイプ（v0.1.0）**。対応する文法・命令は限定的です。コンパイラはオブジェクトファイルを生成しますが、実行ファイルのリンク・実行は行いません。

## クイックスタート

### 必要な環境

- Rust / Cargo（Rust 2024 Editionに対応したツールチェーン）
- Linux x86-64で実行ファイルを作成する場合：Clangと対応するCランタイム・リンカ
- 初回ビルド時にSashimiのGit依存を取得できる環境

初回はリポジトリをクローンし、作業ディレクトリに移動してください。

```sh
git clone https://github.com/Hokutaka/Zangyo.git
cd Zangyo
cargo build
cargo test
```

`examples/hello.zgy`：

```text
出勤
報告 "おはようございます"
定時退社
```

### Linux x86-64 / WSL

Linux向けにオブジェクトファイル、アセンブリ、出自情報を生成します。

```sh
cargo run -- examples/hello.zgy \
  --target x86_64-unknown-linux-gnu \
  -o target/hello.o \
  --asm target/hello.s \
  --origins target/hello.origins.txt
```

生成したオブジェクトを外部ツールでリンクし、実行します。

```sh
clang -fPIE -pie target/hello.o -o target/hello
./target/hello
```

想定される標準出力：

```text
おはようございます
```

### Windows x64（PowerShell）

Windowsでは、PowerShellに貼り付けられる1行のコマンドを使います。Bashの行継続記号（バックスラッシュ `\`）はPowerShellでは使えません。PowerShellで複数行に分ける場合、各行末にバッククォート（U+0060）を置き、直後に空白を入れないでください。

リンクにはVisual Studio / Build ToolsのC++開発ツール（Windows SDK、MSVCのライブラリとリンカ）とClangなどのツールチェーンが必要です。必要に応じて「Developer PowerShell for VS」でZangyoのディレクトリを開いてください。

```powershell
cargo run -- examples/hello.zgy --target x86_64-pc-windows-msvc -o target/hello.obj --asm target/hello.s --origins target/hello.origins.txt
```

生成される `target/hello.obj` はCOFFオブジェクトファイルであり、単体では実行できません。外部リンカが使える環境では、例えば次のようにリンクします。

```powershell
clang --target=x86_64-pc-windows-msvc target/hello.obj -o target/hello.exe
.\target\hello.exe
```

`clang-cl`（ClangのMSVC互換ドライバ）が使える場合は、次の方法もあります。

```powershell
clang-cl target\hello.obj /Fe:target\hello.exe
.\target\hello.exe
```

実行ファイルのリンクには、Windows SDKと適切なCランタイムの設定が必要です。リンクエラーやツール未検出が発生した場合は「Developer PowerShell for VS」で再試行してください。

#### 日本語出力が文字化けする場合

`報告`の出力文字列はUTF-8です。VS CodeのPowerShellターミナルなどで文字化けするときは、同じターミナル内で次を実行してください。

```powershell
chcp 65001
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)
.\target\hello.exe
```

`chcp 65001`だけでは改善しない場合があります。これらの設定は現在のPowerShellセッションに対するものです。

アセンブリと出自情報を確認するには、次のコマンドを実行します。

```powershell
Get-Content target\hello.s
Get-Content target\hello.origins.txt
```

## 対応する構文

| 構文 | 動作 |
| --- | --- |
| `出勤` | プログラムの開始宣言。最初の有効行に必要 |
| `報告 "文字列"` | 外部Cランタイム関数`puts`による文字列の出力（末尾に改行が付加されます） |
| `報告 式` | 符号付き64ビット整数の計算結果を`printf`で表示 |
| `変数名 = 式` | 符号付き64ビット整数の変数へ計算結果を代入 |
| `残業 {` ～ `}` | ブロックを無限に反復 |
| `定時退社` | `main`関数から終了コード0を返す |
| `有給` | 実行時には何も行わず、出自情報のマーカーを追加 |
| `過労死` | x86-64の`ud2`命令を生成（実行時に不正命令例外が発生します） |
| `// ...` | 行頭のコメント（行全体を無視） |

構文上の制約：

- 文は1行につき1つ記述します。ブロックの開始`残業 {`と終了`}`はそれぞれ単独の行にします。
- 空行と、先頭の空白を除いて`//`から始まる行は無視します。行末コメントは未対応です。
- `報告`の文字列はダブルクォートで囲み、`\n`、`\t`、`\r`、`\\"`、`\\`を使用できます。埋め込みNUL文字は扱えません。
- `出勤`以降の文は省略できます。末尾まで制御が到達したときのために、暗黙の終了処理を生成します。
- `定時退社`、`残業`、`過労死`の後に続く、構文上到達できない文には`warning[Z001]`を出力します。警告は簡易的な構文解析に基づき、制御フローの完全な解析ではありません。

### 整数計算の例

`examples/salary.zgy`：

```text
出勤
基本給 = 250000
残業時間 = 40
残業代 = 残業時間 * 1500
給料 = 基本給 + 残業代
報告 給料
定時退社
```

計算結果：

```text
310000
```

演算子は `+`、`-`、`*`、`/`（符号付き整数除算）、単項 `-`、丸括弧です。`*` と `/` が `+` と `-` より優先されます。変数は先に代入してから参照する必要があります。整数は符号付き64ビット（`i64`）で、オーバーフロー検出はありません（マシン命令の挙動に従います）。0による除算や、除算結果が64ビットを超える場合はCPU例外が発生します。式の評価はコンパイル時の定数計算ではなく、x86-64命令として実行時に行います。

Windows PowerShellでこの例を動かす場合：

```powershell
cargo run -- examples/salary.zgy --target x86_64-pc-windows-msvc -o target/salary.obj --asm target/salary.s --origins target/salary.origins.txt
clang --target=x86_64-pc-windows-msvc target/salary.obj -o target/salary.exe
.\target\salary.exe
```

日本語出力が文字化けする場合の設定は上のWindowsの説明を参照してください。

### ループの例

`examples/overtime.zgy`：

```text
出勤
残業 {
    報告 "帰りたい"
}
定時退社
```

`残業`ブロックには終了条件がなく、後続の`定時退社`には到達しません。コンパイラは`warning[Z001]`を標準エラー出力へ表示します。

**注意：この例を実行すると、終了するまで文字列の出力を繰り返します。** 実行による確認では、停止方法やリソース消費に注意してください。

## CLI

```text
zangyo <source.zgy> --target <target-triple> -o <object-path>
  [--asm <asm-path>] [--origins <origins-path>]
```

| 引数 | 説明 |
| --- | --- |
| `<source.zgy>` | 入力ソースファイル |
| `--target` | 対象ターゲット。必須 |
| `-o` | 生成するオブジェクトファイルの保存先。必須 |
| `--asm` | AT&T形式のアセンブリを保存するパス。省略可能 |
| `--origins` | 出自情報をテキストとして保存するパス。省略可能 |

対応ターゲット：

- `x86_64-unknown-linux-gnu`（ELF64）
- `x86_64-pc-windows-msvc`（COFF）

Zangyo自体にリンカや実行環境は含まれません。

## コンパイル処理

```text
Zangyo source (.zgy)
  |
  v
Parser / AST
  |
  v
Sashimi typed Program + OriginId
  |--------------------|-----------------------|
  v                    v                       v
ELF64 / COFF Object    AT&T Assembly           出自情報
```

Zangyoが構文解析・命令選択・呼出規約の処理を担当し、Sashimiが命令の検証、符号化、シンボル参照・再配置、オブジェクトファイル生成を担当します。標準出力には外部の`puts`を利用しています。

出自情報では、ソース行番号を`OriginId`として付与し、生成物の各エントリと関連付けています。`--origins`で出力するオフセットは、**Sashimiが生成したオブジェクトの各セクション内のバイト範囲**です。ファイル全体のオフセットやリンク後の実行時アドレスではありません。

Sashimiは`Cargo.toml`で特定のGitコミットに固定しています。Cerune言語本体のIRやランタイムへの依存はありません。

## 現在の制限

- ユーザー定義関数、条件分岐、有限ループは未対応です。変数はi64のみで、整数演算以外の型や式は未対応です。
- 構文は限定的な行単位の形式で、一般的な式言語のパーサーではありません。
- 文字列の`報告`は`puts`、整数の`報告`は`printf`に依存します。独立したランタイムやOSシステムコールの実装はありません。
- 最適化、完全な到達可能性解析、一般的なデバッグ情報の生成は未対応です。
- Linux/Windows以外のターゲットおよびx86-64以外のアーキテクチャは未対応です。
- 公開APIと診断形式は変更される可能性があります。

## 開発とテスト

```sh
cargo fmt -- --check
cargo test
```

GitHub Actions CIでは、LinuxとWindowsでRustのテストを実行し、LinuxではELFオブジェクトのリンクと実行、WindowsではCOFFオブジェクトの生成を確認する構成です。結果は[Actions](https://github.com/Hokutaka/Zangyo/actions)を参照してください。

テストコードには、基本的なELF/COFF生成、構文エラー、到達不能警告の確認が含まれています。**実際のビルド、テスト通過、リンク後の動作は環境ごとに確認してください。** 生成物が実際に実行可能であることをこれらのテストだけで保証するものではありません。

変更の進め方は[CONTRIBUTING.md](CONTRIBUTING.md)を参照してください。

## ライセンス

本プロジェクトは **MIT License または Apache License 2.0** のデュアルライセンスです。利用者はいずれかのライセンスを選択できます。詳細は [LICENSE-MIT](LICENSE-MIT) および [LICENSE-APACHE](LICENSE-APACHE) を参照してください。
