# Kika

[English](README.md) | **日本語** | [简体中文](README_zh.md)

> この日本語版は`README.md`（英語版）のスナップショット翻訳です。最新の正確な情報は常に英語版を正としてください。

**Kika — Rustのための堅牢な計算幾何ライブラリ。** CMake、Boost、あるいは重いGMP/MPFR依存なしに堅牢な幾何述語（geometric predicates）を求める開発者のための、[CGAL](https://www.cgal.org/)の代替となる純粋Rust・メモリセーフな実装です。

Kika（「幾何」）は、堅牢な2D/3D計算幾何を目指して構築されているRustライブラリです：適応的/厳密フォールバック演算を備えた厳密述語、そして後のフェーズでは、その基盤の上に構築される三角形分割・凸包・多角形アルゴリズム。

状態: **pre-alpha（Phase 1-5およびPhase 6A-6D完了）。** 0.7.1時点で、Kikaは厳密述語、2D凸包、Delaunay三角形分割、制約付きDelaunay三角形分割（狭いスコープ）、単純多角形の三角形分割（穴あり・なし両対応）、Voronoi図（トポロジーと頂点/辺の幾何の両方）、そして点位置判定（point location）を備えた堅牢な2Dカーネルです — 実際にカバーする範囲・カバーしない範囲の詳細は[今日実装されている機能](#implemented-today)と下記の[成熟度](#maturity)の表を参照してください。まだ安定性の保証はありません。まだ存在しないものについては[ロードマップ](#roadmap)を参照してください — **KikaはCGALの代替として完成したものではありません**、将来そうしたものが構築される土台となる堅牢なカーネルです。

## <a id="why-not-just-use-cgal"></a>なぜCGALをそのまま使わないのか？

CGALは計算幾何の成熟した包括的なリファレンス実装です。Kikaの計画は、開発中に自身の述語層をCGALを外部オラクルとして検証することです（§10）— **まだ実施されていません**：比較プログラムは未構築で、現在は環境的にブロックされています（このプロジェクトの開発環境にはCGAL/pkg-configが利用できません）。詳細は[`docs/compatibility.md`](docs/compatibility.md)と[`tasks/todo.md`](tasks/todo.md)を参照してください。現時点の厳密性の主張は、CGALではなく独立した`num-bigint`/`num-rational`オラクルに対して検証されています（下記の[今日実装されている機能](#implemented-today)を参照）。RustプロジェクトにCGALを組み込むこと自体が、C++ツールチェーン、CMake、Boost、そして通常はGMP/MPFRを意味します — `cargo build`のみで完結するワークフロー、WASMターゲット、そして純粋なRust依存関係ツリーを望むチームにとって、これは現実的な障壁です。

Kikaの賭け、順を追って：

| | CGAL | Kika |
|---|---|---|
| 言語 | C++ | Pure Rust |
| ビルド | CMake + Boost | `cargo build` |
| 大数値ライブラリ依存 | GMP/MPFR（通常必須） | ランタイムでは不要（devのみ、テストオラクル用） |
| メモリ安全性 | 手動管理 | コンパイラによって強制（デフォルトで`unsafe`禁止、`docs/architecture.md`参照） |
| WASMターゲット | 実用的でない | `wasm32-unknown-unknown`、CIで確認済み |
| ライセンス | GPL / 商用 | MIT OR Apache-2.0 |
| 今日の機能の広さ | 非常に大きく、何十年もの成熟度 | 意図的に小さい — [今日実装されている機能](#implemented-today)を参照 |

メッシュBoolean演算、NURBS、あるいは今すぐCADカーネルが必要なら、それはKikaではなくCGALです。純粋Rustで構築するための小さく堅牢でpanicしない述語層が欲しいなら、それがKikaのPhase 1です。

## <a id="implemented-today"></a>今日実装されている機能

* `Point2`、`Point3` — 有限座標の点（`f64`）。構築時にNaN/infinityを検証・拒否します。一度構築されれば常に有限です。等価性は厳密な座標の等価性です（許容誤差なし）— ADR-003を参照。
* `Vector2`、`Vector3` — 有限座標の変位ベクトル。標準的な点/ベクトルのアフィン演算を持ちます（`Point ± Vector -> Point`、`Point - Point -> Vector`、ベクトルの`+`/`-`/`-`（否定）/`* f64`）。
* `Segment2`、`Triangle2`、`Triangle3`、`Aabb2`、`Aabb3` — 上記の上に構築された単純なデータ型（`Point2`/`Point3`が既に保証する以上の追加検証はありません）。長さゼロの線分や退化した（頂点が共線の）三角形は、拒否されない有効な表現可能な値です。`Aabb2`/`Aabb3`は、厳密で`orient2d`を使わない`overlaps()`高速棄却テストを提供します。
* `Segment2::relation_to`、`Triangle2::orientation`/`relation_to` — `orient2d`のみから構築された、点と線分・点と三角形の厳密な述語です。各退化ケース（長さゼロの線分、共線の三角形）は明示的に処理されており、一般アルゴリズムから自動的に導かれると想定していません — 実際に1つのケースは当初そうなっておらず、テストによって発見されました。`docs/degeneracy-policy.md`を参照。`tests/differential/`内の独立した厳密有理数オラクルに対して検証済みです。
* `segment_intersection_kind` / `segment_intersection` — 堅牢な2D線分交差判定。分類と座標構築は意図的に別々の関数として保たれています（§4.2）：分類（`None`/`Proper`/`EndpointTouch`/`CollinearTouch`/`CollinearOverlap`）は決して除算や新しい座標の構築を行わず、述語呼び出しの前に`Aabb2`ベースの高速棄却が行われます。構築はすべてのケースで厳密であり、`Proper`（真に新しい座標が必要な唯一のケースで、Phase 5時点で正しく丸められています）も含まれます — [厳密述語 vs 厳密構築](#exact-predicates-vs-exact-constructions)を参照。`tests/differential/`内の独立した厳密有理数オラクルに対して検証済みです。
* `Polygon2` — 頂点のリング。暗黙的に閉じています（最初と最後の頂点の重複なし）。`signed_area()`（プレーンな`f64`、構築）と`orientation()`（厳密 — 実行中の`f64`合計ではなく、核となる述語が使うのと同じ厳密展開機構を用いて各辺のシューレース項を合計）は、他の箇所と同じ分離方針により意図的に分けられています。`basic_validity()`は安価な構造チェック（頂点数、連続する重複頂点、面積ゼロ）をカバーし、`find_self_intersection()`は隣接しない辺間の別個のO(n²)チェックです（隣接する辺が共有する頂点は、正しく自己交差として報告されません）。
* `Sign`、`Orientation` — 述語が返す意味のある列挙型です（生の行列式や曖昧な`bool`ではありません）。
* `orient2d` — 厳密符号の2D方向性述語。*計算された*誤差境界（固定のイプシロンではない）を持つ高速な浮動小数点フィルタを使用し、フィルタが結論を出せない場合は厳密展開演算にフォールバックします。`tests/differential/`内の独立した厳密有理数オラクルに対して検証済みです。[`docs/numerical-model.md`](docs/numerical-model.md)を参照。
* `orient3d` — 厳密符号の四面体方向性述語。`orient2d`と同じフィルタ＋厳密フォールバック設計です。`tests/differential/`内の独立した厳密有理数オラクルに対して検証済みです。
* `incircle` — 厳密符号の外接円内点判定述語。同じフィルタ＋厳密フォールバック設計ですが、多項式の次数が高いため`orient2d`/`orient3d`より狭い検証済み安全座標マグニチュード範囲（`~1e-70`〜`~1e70`）です — [`docs/numerical-model.md`](docs/numerical-model.md)を参照。`tests/differential/`内の独立した厳密有理数オラクルに対して検証済みです。
* `insphere` — 厳密符号の外接球内点判定述語、`incircle`の3D版です。同じフィルタ＋厳密フォールバック設計ですが、`incircle`よりさらに狭い検証済み安全座標マグニチュード範囲（`~1e-30`〜`~1e30`）です — [`docs/numerical-model.md`](docs/numerical-model.md)を参照。`tests/differential/`内の独立した厳密有理数オラクルに対して検証済みです。

* `convex_hull2` / `HullBoundaryPoints` — Andrewのモノトーンチェーンによる2D凸包。`ExtremesOnly`（デフォルト）は厳密な角のみを保持し、`KeepAllOnBoundary`は隣接点と共線な境界上の点も保持します。反時計回りの出力で、辞書順で最小の入力点から開始し、入力順序に依存しません。重複する入力点は最初に取り除かれます。完全に厳密です — 返される各頂点は元の入力`Point2`からコピーされます。アルゴリズムは補間や除算を一切使わず完全に`orient2d`のみから構築されているためです。退化した入力（0/1/2点、すべて共線）は一般アルゴリズムに任せず明示的に処理されます — [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)を参照。

* `delaunay2` / `Triangulation2` — Bowyer-Watson逐次挿入法による2D Delaunay三角形分割。`convex_hull2`と同様に完全に厳密です：「三角形分割の外側」は合成された境界三角形ではなく、単一のシンボリックなゴースト頂点（座標を持たない）で表現されるため、回避すべきスケール依存のトレードオフが存在しません — スパン`10.0`に対する垂直方向のクラスタ広がり`1e-200`まで検証済みです。共円点は複数の有効な三角形分割の間の真の同点であり、唯一の「正しい」答えではありません。決定論的なタイブレークルールは[`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)に他のすべての退化ケース（共線な境界点、既存の辺上に厳密に存在する点）とともに文書化されています。
* `Triangulation2`の隣接構造 — `VertexId`/`EdgeId`/`FaceId`と`vertices`/`edges`/`faces`/`edge_vertices`/`adjacent_faces`/`face_vertices`/`neighboring_faces`/`boundary_edges`。インデックス化された三角形隣接構造の**静的で構築後のスナップショット**です（ADR-006の比較に基づき、half-edge/quad-edgeの汎用性はありません）。`triangles()`は元の座標のみの契約を変更せず維持し、新しいメソッドは純粋に追加的です。
* `constrained_delaunay2` / `ConstrainedTriangulation2` — 2D制約付きDelaunay三角形分割。意図的に狭いスコープです（Phase 6C）：既存の入力頂点間の交差しない制約辺のみで、自動的な交差点/Steiner点生成やリファインメントはありません。このクレート自身の`orient2d`/`incircle`/`segment_intersection_kind`述語を通じて既存のDelaunay辺をフリップすることだけで完全に構築されています — ADR-004のPhase 6再評価は、CDTには**新しい構築が不要**であると予測しており、実装はそれを裏付けています：新しい座標は一つも構築されません。制約の回復とDelaunayの復元はそれぞれ有界です（無限ループになりません）。`CdtError`は交差/共線な制約、アルゴリズムの限界超過、そして退化した点集合(点が3個未満、またはすべて共線)を型付きエラーとして報告し、panicは発生しません。
* `triangulate_polygon` — 単純多角形の三角形分割（Phase 6D）。Phase 6CのCDTの上に構築されています：多角形のすべての辺を制約とし、（非凸な入力に対しては）多角形の外側にある凹んだポケット面を、1つの内部シード面からの純粋にトポロジカルなフラッドフィルによって破棄します — セントロイドのような構築された座標は決して使いません。Steiner点なし（すべての出力頂点は多角形自身の頂点のいずれかです）、自己交差する入力は型付きの`PolygonTriangulationError`として拒否され、CCWとCWの両方の入力を受け付け、決定論的です。完全なスコープの表については[`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)を参照してください。`Triangulation2::validate_topology()`で結果をチェックする際の注意点も含まれます（そのオイラー標数チェックは三角形分割が凸包全体をカバーしていることを前提としていますが、非凸多角形の三角形分割は意図的にそれを持ちません）。
* `triangulate_polygon_with_holes` — 穴付き多角形三角形分割。`triangulate_polygon`自身のアルゴリズムを新しいものに置き換えるのではなく一般化したものです：穴の境界は、同じフラッドフィルが止まる、より多くの制約辺にすぎません。あるホールが別のホールの中にネストしている場合はスコープ外です（部分的なサポートではなく型付きエラー）。それ以外の拒否される入力（境界の外にあるホール、境界に接触・交差するホール、他のホールと接触・交差するホール）も同様に型付きの`PolygonTriangulationError`として扱われ、panicにはなりません。`Polygon2::relation_to`/`PointPolygonRelation`（これと同時に追加された厳密な点と多角形の述語）がホール包含チェックを支えています。
* `Voronoi2` / `voronoi2` — Voronoi図。既存の`Triangulation2`の双対であり、トポロジー（0.5.0）に加えて頂点/辺の幾何（0.7.0）を備えます。クリッピングと最近傍クエリはまだ実装されていません（意図的に後回し）。上記のDelaunay自身の共円タイブレークは1つの共円クラスタを2つ以上の三角形に分割し得ますが、`voronoi2`は`incircle(...) == Sign::Zero`をキーとするunion-findで該当するfaceを統合し、その恣意的な選択が余分なVoronoi頂点や辺として漏れ出さないようにします — 同一の共円点集合を複数の異なる三角形分割に通し、単に同型であるだけでなく同一の出力になることを確認して検証済みです。トポロジーのクエリAPI：`cells`/`vertices`/`edges`、`cell_site`、`neighboring_cells`、`cell_is_unbounded`、`edge_cells`、`edge_kind`、`dual_delaunay_edge`、`vertex_delaunay_faces`、そして`cell_edges`（cell境界を反時計回りに辿る順序付きの巡回。bounded/内部siteのcellは閉じた循環、unbounded/hull siteのcellは2本のrayの間の線形な並び）。既存の`Triangulation2`のface隣接構造だけから構築されており、新しいデータモデルはありません。[`docs/adr/ADR-007-voronoi-diagram-topology.md`](docs/adr/ADR-007-voronoi-diagram-topology.md)を参照。

  `vertex_point`/`edge_geometry`（0.7.0）は、この上に実際の座標を追加します：`vertex_point`はVoronoi頂点が統合するDelaunay faceグループの、正しく丸められた（ADR-004方式の）外心です — 共円で統合されたグループでは、すべてのメンバーfaceが1つの真の外心を共有するため、結果はどのメンバーfaceが計算したかに関わらず証明可能に同一であり、実際に使われるメンバーは構築順ではなく、標準的なsite同一性キーによる規則で選ばれます。`edge_geometry`は、bounded `Segment`またはunbounded `Ray`（正規化されていない外向き方向 — このクレートにはどこにも`sqrt`/normalizeがありません）を返し、opposite-signで`f64::MAX`付近の座標を含む、2つの異なる有限なDelaunay頂点に対して常に有限かつ非ゼロであることが保証されています。faceの真の外心が表現できない場合は`Err(VoronoiGeometryError::NonFiniteCircumcenter)`（`line_intersection`と異なり、これは再スケーリングでは解決できません — この発散は座標のマグニチュードではなく三角形のアスペクト比によって引き起こされます）。このクレート自身の構築が決して破らない内部不変条件については`Err(InvalidTopology)`。どちらの場合もpanicにはなりません。[`docs/adr/ADR-009-voronoi-geometry.md`](docs/adr/ADR-009-voronoi-geometry.md)を参照。
* `Triangulation2::locate` / `PointLocation` — 点位置判定（0.6.0）：`PointLocation::{Vertex(VertexId), Edge(EdgeId), Face(FaceId), Outside}`、閉じた列挙型です（`#[non_exhaustive]`ではありません — この4つのバリアントは`Triangulation2`自身の既に閉じたid語彙の閉包に、必要な「該当なし」ケースを加えたものそのものだからです）。`Outside`は「凸包の外」ではなく「どのfaceにも属さない」ことを意味します — `triangulate_polygon_with_holes`の穴の内部にある点も、それを覆うfaceがないため`Outside`です。計算量は`O(F)`（全faceの線形走査であり、空間indexではありません）。性能はこのリリースの契約に意図的に含めていないため、シグネチャを変更せずに後からより高速なlocatorへ置き換えられます。空のtriangulationを含め、決してpanicしません。crate自身の`Triangle2::relation_to`/`Segment2::relation_to`だけでなく、face全体にわたる集約・分岐ロジック自体を独立に検証するBigRational oracleに対して検証済みです。[`docs/adr/ADR-008-point-location.md`](docs/adr/ADR-008-point-location.md)を参照。

4つの述語すべてがv0.1の堅牢述語スコープを完了しており、上記のプリミティブ・交差判定・多角形・凸包・Delaunay三角形分割はPhase 2からPhase 4を完了しています。`segment_intersection`の`Proper`交差点構築（下記）でPhase 5が完了し、上記の隣接構造・制約付きDelaunay三角形分割・単純多角形三角形分割でPhase 6A-6Dが完了しています。Voronoi図の*トポロジー*（0.5.0）と頂点/辺の*幾何*（0.7.0）、そして点位置判定（0.6.0）はすべて実装済み（上記）です。これより先 — 多角形Boolean演算、空間index/walking locator、最近傍クエリ、そして特にVoronoiクリッピング — は後の話です — [ロードマップ](#roadmap)を参照。

* `predicates::line_intersection`（`segment_intersection`の`Proper`ケースで内部的に使用）— このクレート初の厳密/認証された**構築**です（ADR-004準拠）。真の交差座標に最も近い正しく丸められた（同点の場合は最近偶数丸め）`f64`を返します。近似値ではありません — IEEE-754が単一の算術演算に対して行う保証を、幾何構築全体に拡張したものです。`Point2`はプレーンな`f64`のペアのままです。新しい公開型も新しい依存関係もありません。マグニチュードのスケール、混在マグニチュードの入力、経験的なフロアスイープにわたる独立した`BigRational`「これは正しく丸められた最近傍の`f64`か」オラクルに対して検証済みです — [`docs/numerical-model.md`](docs/numerical-model.md)を参照。

## <a id="exact-predicates-vs-exact-constructions"></a>厳密述語 vs 厳密構築

Kikaの述語（`orient2d`など）は数学的に正しい**符号**を保証します。生成された座標が厳密であることを保証するのは別の問題（「構築」）です — Phase 5時点で、1つのケースが解決されています：`segment_intersection`の`Proper`交差点は、以前の素朴な`f64`補間とは異なり、現在は正しく丸められた構築（`predicates::line_intersection`、ADR-004）です。この関数が返しうる他のケース（`EndpointTouch`/`CollinearTouch`/`CollinearOverlap`）は、元の入力座標を直接再利用することで既に厳密でした。[`docs/architecture.md`](docs/architecture.md) §4.2とADR-004を参照してください。後のフェーズ（Phase 6：制約付きDelaunay、多角形Boolean）で実装された構築が、それぞれのドキュメントがそう述べるまで同じ厳密性の保証を持つとは想定しないでください。

## 退化ケース

共線/共平面/共円/共球な点、重複する点、符号付きゼロ、非正規化座標は明示的に処理・テストされており、「無視してよいほど稀」として扱われることはありません。[`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)を参照してください。

## 最小限の例

```rust
use kika::{Point2, orient2d, Orientation};

let a = Point2::new(0.0, 0.0).unwrap();
let b = Point2::new(1.0, 0.0).unwrap();
let c = Point2::new(0.0, 1.0).unwrap();

assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
```

制約付きDelaunay — フリップしなければ通常はDelaunayの選択となる場合でも、すべての制約辺は結果に確実に含まれます（このスニペットは`cargo test --doc`でdoctestとして検証されています。[`constrained_delaunay2`自身のドキュメント例](src/triangulation/cdt.rs)として存在します）：

```rust
use kika::{Point2, constrained_delaunay2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(4.0, 4.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let constraints = [(0, 2)]; // one diagonal of the square
let cdt = constrained_delaunay2(&pts, &constraints).unwrap();

let constrained_edge_count = cdt
    .triangulation()
    .edges()
    .filter(|&e| cdt.is_constrained(e))
    .count();
assert_eq!(constrained_edge_count, constraints.len());
```

単純多角形の三角形分割 — Steiner点なし、上記の制約付きDelaunayの上に構築されています（こちらもdoctestされています。[`triangulate_polygon`自身のドキュメント例](src/triangulation/polygon.rs)として存在します）。`triangulate_polygon`は境界リング1つのみを扱います（穴なし）。`triangulate_polygon_with_holes`は同じアルゴリズムを、境界リングとゼロ個以上の穴リングへ拡張したものです（Steiner点なし、新しい構築もなし、という点は変わりません）：

```rust
use kika::{Point2, Polygon2, triangulate_polygon};

let square = Polygon2::new(vec![
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(4.0, 4.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
]);
let t = triangulate_polygon(&square).unwrap();

// A simple polygon triangulated with only its own vertices always has
// exactly `polygon.len() - 2` triangles.
assert_eq!(t.len(), square.len() - 2);
```

Voronoi図のトポロジー — `Triangulation2`の双対（こちらもdoctestされています。[`voronoi2`自身のドキュメント例](src/triangulation/voronoi.rs)として存在します）：

```rust
use kika::{Point2, VoronoiEdgeKind, delaunay2, voronoi2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let voronoi = voronoi2(delaunay2(&pts));

// One cell per site, one Voronoi vertex (the triangle's circumcenter),
// and 3 unbounded rays -- no interior Delaunay edge to exclude.
assert_eq!(voronoi.cells().count(), 3);
assert_eq!(voronoi.vertices().count(), 1);
for edge in voronoi.edges() {
    assert!(matches!(
        voronoi.edge_kind(edge),
        VoronoiEdgeKind::Unbounded { .. }
    ));
}
```

Voronoi図の幾何 — 上記のトポロジーに実際の座標を加えたもの（こちらもdoctestされています。[`vertex_point`自身のドキュメント例](src/triangulation/voronoi.rs)として存在します）：

```rust
use kika::{Point2, delaunay2, voronoi2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let voronoi = voronoi2(delaunay2(&pts));
let vertex = voronoi.vertices().next().unwrap();

// The right triangle's circumcenter is its hypotenuse's midpoint.
let p = voronoi.vertex_point(vertex).unwrap();
assert_eq!((p.x(), p.y()), (2.0, 2.0));
```

点位置判定 — `O(F)`、空間indexなし（こちらもdoctestされています。[`locate`自身のドキュメント例](src/triangulation/locate.rs)として存在します）：

```rust
use kika::{Point2, PointLocation, delaunay2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let t = delaunay2(&pts);

// Every input point locates to its own vertex.
let (v0, _) = t.vertices().next().unwrap();
assert_eq!(t.locate(pts[0]), PointLocation::Vertex(v0));

// A point strictly inside the triangle locates to its one face.
assert!(matches!(
    t.locate(Point2::new(1.0, 1.0).unwrap()),
    PointLocation::Face(_)
));

// A point outside the hull.
assert_eq!(t.locate(Point2::new(10.0, 10.0).unwrap()), PointLocation::Outside);
```

その他、`cargo run --example <name>`で実行できる例が[`examples/`](examples/)にあります：

* [`orient2d`](examples/orient2d.rs) — 基本的な向き判定述語
* [`segment_intersection`](examples/segment_intersection.rs) — 線分交差の分類と構築
* [`convex_hull`](examples/convex_hull.rs) — `ExtremesOnly` vs `KeepAllOnBoundary`
* [`delaunay`](examples/delaunay.rs) — 2D Delaunay三角形分割
* [`polygon_validity`](examples/polygon_validity.rs) — `basic_validity`と`find_self_intersection`
* [`constrained_delaunay`](examples/constrained_delaunay.rs) — 特定の（Delaunayでない可能性のある）辺を強制的に残す
* [`polygon_triangulation`](examples/polygon_triangulation.rs) — 非凸多角形、三角形数/CCW/面積のチェック付き
* [`polygon_triangulation_with_holes`](examples/polygon_triangulation_with_holes.rs) — 2つの独立した穴をくり抜いた境界
* [`voronoi`](examples/voronoi.rs) — 共円な正方形とオフセンターの内部点、bounded/unboundedなcell、さらに（0.7.0）各cell境界の実際のsegment/ray幾何
* [`locate`](examples/locate.rs) — vertex/edge/face/outsideの分類、穴の内部と境界を含む

## WASM

述語コアはOS依存やプラットフォーム固有のコードを持たず、`wasm32-unknown-unknown`向けにビルドできます。これはCIで確認されています。WASM専用のバインディング（`wasm-bindgen`など）はまだ存在しません。

## CGALとの違い

KikaはCGALをリンクせず、ソースコードも共有していません。CGALは開発中の外部の別個の差分テストオラクルとしてのみ使用される*計画*です（プロジェクトの開発指示書§10）— `kika`クレート自体のランタイムやビルド時の依存関係になることは決してありません — ただし、その比較プログラムはまだ存在しません。上記の[なぜCGALをそのまま使わないのか？](#why-not-just-use-cgal)を参照してください。

## 安定性

Pre-1.0であり、semverの保証はありません。一部の計算幾何ライブラリ（CGALを含む）に見られる公開`Kernel`トレイトの設計は、意図的にまだ確定していません — ADR-004を参照。0.3.0時点で、公開されている`Result`型のエラーenum（`KikaError`、`CdtError`、`PolygonTriangulationError`、そして0.7.0時点で`VoronoiGeometryError`）は`#[non_exhaustive]`になっており、将来variantが追加されても、既にワイルドカードアームを持つ呼び出し側の`match`が壊れることはありません — `CHANGELOG.md`を参照。

## <a id="maturity"></a>成熟度

| 機能 | 状態 |
|---|---|
| 述語（`orient2d`、`orient3d`、`incircle`、`insphere`） | 評価に十分安定 — フィルタ＋厳密フォールバック、独立オラクルに対して検証済み |
| 線分交差判定 | 実装済み — 分類は厳密、`Proper`構築は正しく丸められる（ADR-004） |
| 凸包 | 実装済み — 完全に厳密 |
| Delaunay三角形分割 | 実装済み — 完全に厳密、合成座標なし |
| 三角形分割の隣接関係（頂点/辺/面クエリ） | 実装済み — `VertexId`/`EdgeId`/`FaceId`、隣接/境界クエリ、内部トポロジー検証器（ADR-006） |
| 制約付きDelaunay | 実装済み — 狭いスコープ：既存頂点間の交差しない制約のみ、Steiner点なし（Phase 6C） |
| 単純多角形の三角形分割 | 実装済み — Steiner点なし、自己交差する入力は拒否（Phase 6D）。穴に対応（0.4.0、`triangulate_polygon_with_holes`）— ネストした穴はスコープ外、型付きエラー |
| Voronoi図 | 実装済み — トポロジー（0.5.0）：cells/vertices/edges、順序付き`cell_edges()`境界巡回。頂点/辺の幾何（0.7.0）：`vertex_point`/`edge_geometry`、正しく丸められた外心、正規化されていないray方向。クリッピングと最近傍クエリはまだ |
| 点位置判定 | 実装済み — `Triangulation2::locate`（0.6.0）、`O(F)`の線形走査、独立したBigRational oracleに対して検証済み。空間index/walking locatorや最近傍クエリはまだ |
| 多角形Boolean演算 | 未実装 — 厳密性モデルはまだ未確定、ADR-004を参照 |
| 3Dメッシュ演算 | 未実装 |

## ライセンス

以下のいずれかの下でライセンスされています

* MITライセンス（[LICENSE-MIT](LICENSE-MIT)）
* Apacheライセンス バージョン2.0（[LICENSE-APACHE](LICENSE-APACHE)）

どちらを選ぶかは利用者の任意です。

## <a id="roadmap"></a>ロードマップ

Phase 1（堅牢述語）、Phase 2（2Dプリミティブと交差判定）、Phase 3（2D凸包）、Phase 4（2D Delaunay三角形分割）、Phase 5（認証/厳密構築 — 厳密な`Proper`線分交差点）、Phase 6A-6D（三角形分割の隣接関係、狭いスコープの制約付きDelaunay、狭いスコープの単純多角形三角形分割）、Voronoi図の*トポロジー*（0.5.0）と頂点/辺の*幾何*（0.7.0）、そして点位置判定（0.6.0）が完了しています。まだ実装されていないもの：Voronoiクリッピング・最近傍クエリ、`locate`向けの空間index/walking locator、多角形/メッシュBoolean演算、頂点削除、Delaunayリファインメント、メッシュ修復、サーフェス再構築、点群処理。フェーズ分けされたバックログについては[`tasks/todo.md`](tasks/todo.md)を、`crates.io`/GitHubリリース前に検証済みのものについては[`docs/release-checklist.md`](docs/release-checklist.md)を参照してください（0.2.0から0.7.1まですでにリリース済み — `CHANGELOG.md`を参照）。
