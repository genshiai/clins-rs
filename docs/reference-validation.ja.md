# 参照Validatorとの比較と上流試験の再利用

更新：2026-11-19。

## Java版は何か

[HL7 FHIR Java Core / Validator](https://github.com/hapifhir/org.hl7.fhir.core) は、
汎用のJava製FHIR実装・検証器です。CLINS公式は、この検証器へR4・JP Core・用語・CLINSの定義を
読み込ませる [Validation手順](https://jpfhir.jp/fhir/clins/igv1/validationGuide.html) を案内しています。
CLINS専用の日本製検証エンジンが別にあるという意味ではありません。

Kotlin/JVMは [JavaのAPIと相互運用](https://kotlinlang.org/docs/java-interop.html) できるため、
Java ValidatorをKotlinから利用する構成も可能です。その構成の検証エンジンはJavaで、実行にはJVMが必要です。
clins-rsのKotlin SDKは、UniFFI/JNA経由でRust Coreを呼ぶ構成です。

## 現在の比較

`scripts/compare-reference.py` は、次の条件で実行します。

- Java Validator **7.0.6**、jarのSHA-256を固定して照合。
- clins-rs側のR4・JP Core・用語・CLINSの4パッケージを版とSHA-256で確認。
- 患者URLへの取得機能・HTTPアクセス・用語サーバーへの通信を無効化。
- 本リポジトリの架空正常例8件と、その異常変異8件を双方に渡す。
- 期待する判定と、Java側のerror/fatalの有無を比較する。
- エラー箇所を正規化して、共通・Rust側のみ・Java側のみの箇所を記録する。

```sh
python3 scripts/compare-reference.py --java /path/to/java \
  --validator /path/to/validator_cli-7.0.6.jar --cache-home /path/to/reference-cache \
  > data/reference-comparison.json
```

Java側の公開定義キャッシュは事前に準備します。Javaが追加で読み込む標準サポートパッケージもあり、
両者の依存関係が完全に同一とは主張しません。通常の製品利用にJavaは必要ありません。

レポートの `all_agreed` は **期待する判定・エラー有無の一致** です。
エラー箇所や文面が完全に一致したという意味ではありません。
Java側でerror/fatalが出なかった状態は `no-errors` と記録し、検証をすべて完了したという意味の
`valid` と同一視しません。Rust側の `incomplete` を一致済みの正常例へ数えることもありません。

箇所の正規化は、Javaの `/*Patient/id*/` のような診断用コメント、数値添字の表記、
Rustの汎用ルート `Resource` を調整します。異なる項目を指す診断や、親と子の粒度の差は保持します。
本文の完全一致は、言語・実装・版による違いがあるため判定基準にしません。

2026-11-19の再実行では、16件すべてで期待する判定・エラー有無が一致しました。
正規化後のエラー箇所集合には5ケースで差がありました。
たとえばアレルギーのcode欠落では、Rustのみの箇所に `Bundle.entry[1].resource.code`、
Javaのみの箇所に `Bundle.entry[1].resource` がありました。
紹介状・退院時サマリー・患者サマリーでも差を記録しています。
これらがすべて同じ規則違反を異なる粒度で示したものかは、規則ID・適用条件まで追って確認する必要があります。
合否の一致だけで診断内容まで一致したとは数えません。

## 再利用する上流の試験

Java側の [ValidationTests.java（7.0.6）](https://github.com/hapifhir/org.hl7.fhir.core/blob/7.0.6/org.hl7.fhir.validation/src/test/java/org/hl7/fhir/validation/tests/ValidationTests.java) は、
共有コーパスのmanifestを読み、試験ごとの定義・オプションを設定して実行します。
JavaのJUnitコードはJavaのAPIに結び付いていますが、入力FHIR JSON・追加定義・期待結果は
Rust側の試験でも利用する対象になります。

[FHIR/fhir-test-cases](https://github.com/FHIR/fhir-test-cases) の確認に使用したコミットは
`2cfcc672d39fe85dcf358106969cf63e185fd926` です。
その [validator/manifest.json](https://github.com/FHIR/fhir-test-cases/blob/2cfcc672d39fe85dcf358106969cf63e185fd926/validator/manifest.json) では、
`version` が4.0／4.0.1、`file` がJSON、`use-test` がfalseでない項目を数えると **598ケース** あります。
これは候補の棚卸し件数で、実行・合格件数ではありません。現在の16件比較は、この上流コーパスの移植ではありません。

再利用時には次をケースごとに保持します。

| 項目 | 扱い |
|---|---|
| 入力・追加定義・期待結果 | 元のパス、コミット、ファイルのSHA-256、上流ケース名を保持する |
| FHIRとIGの版 | R4 4.0.1と対象範囲を選び、別版や別IGを無条件に読み替えない |
| 実行設定 | 追加packages、supporting/profile、用語、参照解決、examples、best-practice等を引き継ぐ |
| 未対応・対象外 | 実行できない理由を記録し、合格件数に加えない。差分を隠すために除外しない |
| 判定・箇所・規則ID | まずエラー有無を比較し、次にseverity、規則ID、対象箇所まで一致を確認する |
| 不一致 | Rustの不足、Javaの挙動、設定・版の差、仕様解釈の差を調べ、期待結果を独立に確認する |

両プロジェクトはApache-2.0の表示があります：
[Java CoreのLICENSE](https://github.com/hapifhir/org.hl7.fhir.core/blob/7.0.6/LICENSE.txt)、
[共有コーパスのLICENSE](https://github.com/FHIR/fhir-test-cases/blob/2cfcc672d39fe85dcf358106969cf63e185fd926/LICENSE.txt)。
取り込む際は、各ファイルの出典・変更点・著作権表示・LICENSE／NOTICEと、
用語など個別コンテンツの条件を確認して保持します。

上流の基本FHIR試験に加えて、CLINS固有の公式作例・規則別の正常／異常試験も必要です。
上流で期待される挙動と一致したことは有力な検証根拠になりますが、国サービスの接続試験や
すべてのCLINS要件の証明とは分けて記録します。

比較レポートの集計・箇所正規化の回帰試験：

```sh
python3 -m unittest discover -s scripts/tests
```
