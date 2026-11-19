# 2026-11-19：上流R4試験とCLINSの差分監査

## 結論

元の架空16例について、期待する合否が16/16で一致することは再現できました。
ただし、上流R4の146ケースへ広げると検証不足・過剰な拒否が見つかりました。
**現行alpha.2の実装に問題がない、という結論にはなりません。**

本監査ではCoreを修正して結果を良く見せることはしていません。既存バイナリで測定した基準値です。
この文書・再現ツール・既知の不具合の明示を追加しました。Coreの修正完了を意味しません。

## 上流146ケースの結果

| 分類 | 件数 |
|---|---:|
| 双方ともerror/fatalなし：Rust valid / Java no-errors | 27 |
| 双方とも拒否：Rust invalid / Java errorあり | 38 |
| Javaだけが拒否：Rust valid | 32 |
| Rustだけが拒否：Java errorなし | 6 |
| Rust incomplete | 33 |
| Rustが入力段階で拒否（ValueError） | 10 |
| 合計 | **146** |

判定を返した103件では **65/103 = 63.11%** がエラー有無で一致しました。
全146件に対する、この定義での一致確認済み件数は **65/146 = 44.52%** です。
未完了33件と入力拒否10件を隠して、146件がすべて判定できたようには数えません。
入力拒否10件はJavaと上流記録でもエラーがあるデータでしたが、上表では入力解析と検証を分けています。
タイムアウト・workerクラッシュ・予期しないruntime-errorは今回0件でした。

これは選択した試験群の観測値であり、一般のFHIRデータやCLINSの「正解率」ではありません。
両者が同じ間違いをする可能性もあるため、合否一致だけでは規則の正しさを証明しません。

### 分母と実行条件

- 上流：FHIR/fhir-test-cases `2cfcc672d39fe85dcf358106969cf63e185fd926`。
- R4 4.0／4.0.1、JSON、無効化されていない候補：598ケース。
- 追加設定を要しないケース：146。実行前のmanifestだけで選択し、合格例を選んだものではありません。
- 残る452ケース：追加packages・profile・supporting・設定などが必要なため今回未実行。理由はロックと結果に記録。
- Java Validator：7.0.6、jar SHA-256固定。HTTP・参照先取得・用語サーバー通信は無効。
- Rust：clins-py 0.1.0-alpha.2のネイティブCore。実際のライブラリSHA-256を結果に記録。
- Rustの定義：検証済みhl7.fhir.r4.core 4.0.1のみ。CLINSの自動プロファイル適用を混ぜていません。
- Javaはそのキャッシュにある標準サポートパッケージも利用。上流の完全なJUnit環境と同一ではありません。
- upstream記録の期待エラー有無と、今回のJava実行は **133/146** で一致。13件は異なりました。

用語サービスやサポート定義・Javaの版などによる条件差があるため、32件すべてを
無条件にRustのバグと断定してはいません。以下のように、仕様と照らして絞り込める不足があります。

## 確認できた代表的な不足・差異

| ケース | Rust / Java | 調査内容 |
|---|---|---|
| `resource-invalid-id-1` | valid / error | `Location.id = "/foobar=="` をRustが通す。上流期待結果もerror。Resource.idの字句制約に不足 |
| `resource-invalid-id-2`・`resource-invalid-id-3` | valid / error | 長すぎるid、contained resourceの不正なidも通る |
| `json-empty-primitive-array`と関連3件 | valid / error | `Patient.name[0].given = []` 等が通る。JSON表現の空配列制約に不足 |
| `bundle-duplicate-id`など | valid / error | 相対的なfullUrlをJavaが拒否し、Rustが通す。fullUrlと参照の制約を点検する必要 |
| `contained-resource-duplicate` | valid / error | contained resourceの重複idをRustが検出しない |
| `bundle-document-versioned-references-good` | invalid / no-errors | 異なるmeta.versionIdを持つ同一fullUrlを、Rustが一律に重複として拒否する。上流期待もerrorなし |
| Parameters参照の3ケースなど | invalid / no-errors | ローカル参照解決の範囲・方法に差がある。過剰拒否の調査が必要 |

Resource.idの許容文字・長さは [FHIR R4 primitive id](https://hl7.org/fhir/R4/datatypes.html#id)、
空配列の扱いは [FHIR R4 JSON表現](https://hl7.org/fhir/R4/json.html) で確認できます。
CodeSystem・ValueSet・SearchParameter等の専用チェックにも差があり、完全な内訳は結果ファイルに残しています。

## 元の16例と、エラー箇所が違った5例

| 例 | Rust error数 / Java error数 | 中身を読んだ結果 |
|---|---|---|
| アレルギーのcode欠落 | 2 / 2 | 両者とも必須code欠落とclosed slicing違反を検出。Rustは`.resource.code`、Javaは親の`.resource`を指す |
| 検査のcode欠落 | 4 / 4 | 必須code・closed slicing・ローカルコード条件を両者が検出。親／子の診断位置が異なる |
| 紹介状のsection欠落 | 3 / 6 | 両者ともsection不足・構成条件・Composition slice不足を検出。Javaはさらに到達不能リソースも検出している |
| 退院時サマリーのsection欠落 | 2 / 2 | 両者ともセクション構成の条件とComposition slice不足を検出。slice不足の位置が`.entry`とBundleルートで異なる |
| 患者サマリーのsection欠落 | 2 / 2 | 両者とも必須section・Composition slice不足を検出。親／子の位置が異なり、文字列上の位置集合は一致しない |

error数は重複した継承規則や派生エラーを含むため、検出した独立規則の数ではありません。
5件すべてが5個の実装バグという意味でも、すべてが表示だけの差という意味でもありません。

### 紹介状で不足を独立に再現した追加試験

正常な紹介状のsection等は残し、そのコピーに**参照されていないOrganizationを1件だけ追加**しました。
追加リソース自体は既存の架空Organizationを複製し、idとfullUrlだけを別の有効な値に変えています。

- Rust：`valid`。
- Java：到達不能なOrganizationのerror 1件。
- 元の16例と合わせると17件中16件が期待する判定と一致し、この追加1件は不一致。

FHIR文書に含めるリソースの範囲は [FHIR R4 Documents 3.3.1](https://hl7.org/fhir/R4/documents.html) に記載があります。
現Coreに文書の到達可能性を確認する不足があることを、他の必須項目の欠落と切り離して再現できました。
元の16例では、どちらも別のエラーによってinvalidになったため、合否比較だけではこの不足が隠れていました。

## 再現する

Python SDKを導入し、プロジェクトの公開パッケージ・Javaの公開定義キャッシュを準備します。
患者の情報や、非公開カルテのファイルは使いません。

```sh
# 固定した上流の公開試験データを取得してSHA-256を照合
.venv/bin/python scripts/compare-upstream.py prepare

# 以降の比較はオフライン。利用可能なJavaとキャッシュのパスを指定する
.venv/bin/python scripts/compare-upstream.py run \
  --java /path/to/java --validator /path/to/validator_cli-7.0.6.jar \
  --cache-home /path/to/reference-cache > data/upstream-audit.json

# 差異・未完了・入力拒否も終了コード1にしたい場合は --fail-on-divergence を付ける
# 既定の終了コード0は「監査処理が完了した」の意味で、適合確認済みの意味ではない
```

追加の到達可能性試験：

```sh
python3 scripts/compare-reference.py \
  --java /path/to/java --validator /path/to/validator_cli-7.0.6.jar \
  --cache-home /path/to/reference-cache --include-reachability-probe \
  > data/clins-reachability-audit.json
# 現alpha.2では、この追加試験の不一致により終了コード1になる
```

初回の選択条件・入力・上流期待結果は [upstream-tests.lock.json](../../scripts/upstream-tests.lock.json) に固定しています。
`lock` モードは上流の公開データを取得してロックを更新する明示的な操作です。
通常の再実行では `prepare` と `run` を使います。

## 証跡

- [146件の一覧CSV](2026-11-19-upstream-r4.csv)
- [146件の詳細JSON gzip](2026-11-19-upstream-r4.json.gz)：判定、未完了数、両者のエラー、上流期待結果、バイナリ／入力のSHA-256を保持
- [CLINS 16例＋追加1例のJSON gzip](2026-11-19-clins-seventeen.json.gz)
- [ファイルのSHA-256](SHA256SUMS)

共有コーパスはApache-2.0表示であり、元のLICENSE・出典を保持しています。
上流入力そのものはignored dataへ取得し、Core／SDKの実行時パッケージには同梱しません。

次の修正優先順位は、**基本JSON／idの見逃し、文書到達可能性、重複・参照解決、用語・専用リソース検査**です。
修正後はこの基準値と比較し、単にJavaと一致させるだけでなく、規格上の期待結果を確認します。
