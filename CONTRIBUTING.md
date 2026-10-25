# Contributing to Zangyo

Zangyoは、[Sashimi](https://github.com/Cerune-Lang/Sashimi)の型付きAPIを使用する、実験的な小規模コンパイラです。IssueやPull Requestを歓迎します。

## 変更の進め方

1. READMEと現在の実装を確認してください。
2. バグ修正には再現ケース、機能追加には最小の `.zgy` 例とテストを添えてください。
3. ローカルで `cargo test` を実行し、Rustコードを編集した場合は `cargo fmt` で整形してください。
4. 可能であれば、LinuxとWindowsのObject生成を確認してください。リンクには各OSのツールチェーンが必要です。
5. PRには変更目的、検証範囲と未確認事項を簡潔に記載してください。

## 実装方針

- 言語の名前や構文にユーモアがあっても、README・診断・テストでは正確さを優先します。
- Zangyoは構文・意味・命令選択・ABI、Sashimiは命令符号化・再配置・Object生成を担当します。
- `OriginId` の対応を変更した場合、`--origins` の内容を確認してください。
- Objectの生成と実行ファイルのリンク・実行を区別してください。
- 未実装機能をREADMEで利用可能と案内しないでください。
- 無限ループや `ud2` のサンプルを、制御なしにCIから実行しないでください。

## CIとブランチ保護

`.github/workflows/ci.yml` でWindowsとLinuxのテスト・Object生成確認を行います。

GitHubのブランチ保護は **Settings → Rules → Rulesets** で管理します。CIの設定ファイルを置くだけでルールセットが有効になるわけではありません。必須ステータスチェックを設定する場合、CIが少なくとも一度実行された後で実際のジョブ名を選んでください。
