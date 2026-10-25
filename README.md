# Zangyo 💀

**A programming language where you can't leave work.**

労働はつらい。コンパイラは真面目に作る。

Zangyo（ザンギョー）は日本語の勤怠キーワードから **本物の x86-64 ELF64 / COFF Object** を生成する、ジョーク言語かつ小さなコンパイラ実験です。機械語の生成には [Sashimi](https://github.com/Cerune-Lang/Sashimi) の型付き `Program` APIを使います。

> **実験的な v0.1 プロトタイプ**。現時点の実装は小さな文法だけをサポートします。実行ファイルを作るには外部リンカが必要です。

## まずは出勤

```zgy
出勤
報告 "おはようございます"
定時退社
```

**Rust / Cargo と、Linux x86-64 上の Clang** を用意してください。

```sh
cargo run -- examples/hello.zgy --target x86_64-unknown-linux-gnu -o target/hello.o --asm target/hello.s --origins target/hello.origins.txt
clang -fPIE -pie target/hello.o -o target/hello
./target/hello
```

出力：

```text
おはようございます
```

Windows向け COFF オブジェクトは `--target x86_64-pc-windows-msvc` で生成できます。実行ファイル化には対応する MSVC/Clang 等のリンカ・CRT が別途必要です。

## 文法（現在実装）

| キーワード | 意味 |
| --- | --- |
| `出勤` | ファイルの先頭に必須 |
| `報告 "文字列"` | C ランタイムの `puts` で1行出力。文字列は `\\n`, `\\t`, `\\r`, `\\"`, `\\\\` をエスケープ可能 |
| `残業 {` ... `}` | ブロックを無限に実行 |
| `定時退社` | `main` から0を返す |
| `有給` | 何もしない（出自マーカーだけ残す） |
| `過労死` | `ud2`（不正命令トラップ）。実行注意 |
| `// コメント` | 行全体のコメント |

- 文は**1行1文**で、`残業 {` と `}` はそれぞれ単独行です。
- `出勤` 以外は省略できます。最後まで到達した場合は暗黙の `定時退社` 相当の終了処理を生成します。
- `残業` / `定時退社` / `過労死` の後の到達不能文には `warning[Z001]` を出します。実装は単純な構文上の判定です。
- `報告` は `puts` を使うため、埋め込みNUL文字を拒否します。
- 条件分岐、変数、関数、演算、任意の式はまだありません。

### 退勤できない例

```zgy
出勤
残業 {
    報告 "帰りたい"
}
定時退社
```

```text
warning[Z001]: 6行目の処理には到達できません
  = note: 仕様です
```

`examples/overtime.zgy` に収録しています。**実行すると無限に出力するので注意。**

## 生成物

```text
.zgy source
  → parser
  → Sashimi typed Program (+ source line as OriginId)
  ├─ ELF64 / COFF object     (-o)
  ├─ AT&T assembly           (--asm)
  └─ section-relative ranges (--origins)
  → 外部リンカ              (Zangyo自身には含まれません)
```

`--origins` は各命令・データの出自行と、**Sashimiが生成したObjectのセクション内**バイト範囲を示します。オブジェクトファイル内の絶対位置やリンク後のアドレスではありません。

## 開発

```sh
cargo test
cargo fmt -- --check
```

SashimiはGitリビジョンに固定しています。Rust製ですが、ZangyoはCerune言語のランタイムやIRを必要としません。

## 将来の悪ふざけ（未実装）

- `上司確認 (条件)` — 条件分岐
- `会議 (回数)` — 有限ループ
- `早退` — 関数の途中からreturn
- `有給残日数` — コンパイル時定数？
- 診断のさらなる労働基準法対応（ジョーク）

## License

MIT OR Apache-2.0 を予定。ライセンスファイルの追加まではライセンス条件を確定していません。
