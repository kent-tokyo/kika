# Kika

[English](README.md) | [日本語](README_ja.md) | **简体中文**

> 本中文版是`README.md`（英文版）的快照翻译。最新、最准确的信息请始终以英文版为准。

**Kika — 面向 Rust 的健壮计算几何库。** 一个纯 Rust、内存安全的 [CGAL](https://www.cgal.org/) 替代方案，面向那些希望在不依赖 CMake、Boost 或沉重的 GMP/MPFR 的情况下获得健壮几何谓词（geometric predicates）的开发者。

Kika（「幾何」）是一个致力于健壮 2D/3D 计算几何的 Rust 库：具备自适应/精确回退运算的精确谓词，以及在后续阶段基于此基础构建的三角剖分、凸包与多边形算法。

状态：**pre-alpha（Phase 1-5 及 Phase 6A-6D 已完成）。** 截至 0.7.0，Kika 是一个健壮的 2D 内核，具备精确谓词、2D 凸包、Delaunay 三角剖分、约束 Delaunay 三角剖分（范围有限）、简单多边形三角剖分（含孔洞与不含孔洞）、Voronoi 图（拓扑与顶点/边几何）以及点定位（point location）——具体覆盖了什么、没有覆盖什么，请参见[今天已实现的功能](#implemented-today)和下面的[成熟度](#maturity)表。目前尚无稳定性保证。尚不存在的功能请参见[路线图](#roadmap)——**Kika 并不是一个已完成的 CGAL 替代品**，而是未来可能构建出这样一个替代品的健壮内核。

## <a id="why-not-just-use-cgal"></a>为什么不直接使用 CGAL？

CGAL 是计算几何领域成熟、全面的参考实现。Kika 的计划是在开发过程中将其谓词层与 CGAL 作为外部预言机（oracle）进行对照测试（§10）——**目前尚未完成**：对照程序尚未构建，且当前处于环境受阻状态（本项目的开发环境中没有可用的 CGAL/pkg-config）。详情请参见 [`docs/compatibility.md`](docs/compatibility.md) 和 [`tasks/todo.md`](tasks/todo.md)。目前的精确性主张是针对独立的 `num-bigint`/`num-rational` 预言机验证的（见下方[今天已实现的功能](#implemented-today)），而不是针对 CGAL。将 CGAL 引入 Rust 项目本身就意味着需要 C++ 工具链、CMake、Boost，通常还需要 GMP/MPFR——这对于仅依赖 `cargo build` 的工作流、WASM 目标平台，以及希望保持纯 Rust 依赖树的团队来说，是真实存在的阻碍。

Kika 的赌注，按顺序：

| | CGAL | Kika |
|---|---|---|
| 语言 | C++ | 纯 Rust |
| 构建方式 | CMake + Boost | `cargo build` |
| 大数库依赖 | GMP/MPFR（通常必需） | 运行时不需要（仅开发环境需要，用于测试预言机） |
| 内存安全 | 手动管理 | 由编译器强制保证（默认禁止 `unsafe`，见 `docs/architecture.md`） |
| WASM 目标平台 | 不实用 | `wasm32-unknown-unknown`，已在 CI 中验证 |
| 许可证 | GPL / 商业许可 | MIT OR Apache-2.0 |
| 当前功能广度 | 非常庞大，历经数十年成熟 | 有意保持精简 —— 见[今天已实现的功能](#implemented-today) |

如果你今天就需要网格布尔运算、NURBS 或一个完整的 CAD 内核，那应该选择 CGAL 而不是 Kika。如果你想要一个小而健壮、不会 panic 的谓词层，用纯 Rust 构建，那正是 Kika 的 Phase 1 所提供的。

## <a id="implemented-today"></a>今天已实现的功能

* `Point2`、`Point3` —— 有限坐标的点（`f64`）。构造时会校验并拒绝 NaN/无穷大；一旦构造完成，就始终保持有限。相等性判断是精确的坐标相等（无容差）——见 ADR-003。
* `Vector2`、`Vector3` —— 有限坐标的位移向量，具备标准的点/向量仿射运算（`Point ± Vector -> Point`、`Point - Point -> Vector`，以及向量的 `+`/`-`/`-`（取负）/`* f64`）。
* `Segment2`、`Triangle2`、`Triangle3`、`Aabb2`、`Aabb3` —— 建立在上述类型之上的纯数据类型（除了 `Point2`/`Point3` 已保证的内容外，不做额外校验）；零长度线段和退化的（顶点共线的）三角形都是有效的、可表示的值，不会被拒绝。`Aabb2`/`Aabb3` 提供精确的、不依赖 `orient2d` 的 `overlaps()` 快速排除测试。
* `Segment2::relation_to`、`Triangle2::orientation`/`relation_to` —— 完全基于 `orient2d` 构建的精确的点与线段、点与三角形关系谓词。每一种退化情形（零长度线段、共线三角形）都被显式处理，而不是假设它会从通用算法中自然得出——其中一种情形最初确实没有被正确处理，是通过测试发现的，见 `docs/degeneracy-policy.md`。已针对 `tests/differential/` 中独立的精确有理数预言机进行验证。
* `segment_intersection_kind` / `segment_intersection` —— 健壮的 2D 线段相交判定。分类与坐标构造被有意保持为两个独立的函数（§4.2）：分类结果（`None`/`Proper`/`EndpointTouch`/`CollinearTouch`/`CollinearOverlap`）从不进行除法运算或构造新坐标，且在调用任何谓词之前会先做基于 `Aabb2` 的快速排除。构造对每一种情形都是精确的，包括 `Proper`（唯一需要真正构造新坐标的情形，截至 Phase 5 已实现正确舍入）——见[精确谓词 vs 精确构造](#exact-predicates-vs-exact-constructions)。已针对 `tests/differential/` 中独立的精确有理数预言机进行验证。
* `Polygon2` —— 一个顶点环，隐式闭合（首尾顶点不重复）。`signed_area()`（普通 `f64`，属于构造）与 `orientation()`（精确——使用与核心谓词相同的精确展开机制来累加每条边的鞋带式（shoelace）项，而不是使用运行中的 `f64` 累加）被有意分开，与其他地方保持相同的拆分原则。`basic_validity()` 覆盖了廉价的结构性检查（顶点数量、连续重复顶点、面积为零）；`find_self_intersection()` 是单独的、O(n²) 的、针对非相邻边的检查（相邻边共享的顶点不会被错误地报告为自相交）。
* `Sign`、`Orientation` —— 谓词返回的具有明确语义的枚举类型（而不是原始的行列式值，也不是含糊不清的 `bool`）。
* `orient2d` —— 精确符号的 2D 方向判定谓词。使用带有*计算得出*误差界（而非固定 epsilon）的快速浮点过滤器，在过滤器无法给出结论时回退到精确展开运算。已针对 `tests/differential/` 中独立的精确有理数预言机进行验证。见 [`docs/numerical-model.md`](docs/numerical-model.md)。
* `orient3d` —— 精确符号的四面体方向判定谓词。与 `orient2d` 采用相同的过滤器 + 精确回退设计。已针对 `tests/differential/` 中独立的精确有理数预言机进行验证。
* `incircle` —— 精确符号的外接圆内点判定谓词。采用相同的过滤器 + 精确回退设计，但由于多项式次数更高，其经验证安全的坐标量级范围（约 `1e-70` 到 `1e70`）比 `orient2d`/`orient3d` 更窄——见 [`docs/numerical-model.md`](docs/numerical-model.md)。已针对 `tests/differential/` 中独立的精确有理数预言机进行验证。
* `insphere` —— 精确符号的外接球内点判定谓词，是 `incircle` 的 3D 版本。采用相同的过滤器 + 精确回退设计，其经验证安全的坐标量级范围（约 `1e-30` 到 `1e30`）比 `incircle` 更窄——见 [`docs/numerical-model.md`](docs/numerical-model.md)。已针对 `tests/differential/` 中独立的精确有理数预言机进行验证。

* `convex_hull2` / `HullBoundaryPoints` —— 基于 Andrew 单调链算法的 2D 凸包。`ExtremesOnly`（默认）只保留严格的角点；`KeepAllOnBoundary` 还会保留与相邻点共线的边界点。输出为逆时针方向，从字典序最小的输入点开始，与输入顺序无关；重复的输入点会先被去重。完全精确——每个返回的顶点都直接复制自原始输入的 `Point2`，因为该算法完全基于 `orient2d` 构建，不涉及任何插值或除法。退化输入（0/1/2 个点、全部共线）会被显式处理，而不是交给通用算法——见 [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)。

* `delaunay2` / `Triangulation2` —— 基于 Bowyer-Watson 增量插入法的 2D Delaunay 三角剖分。与 `convex_hull2` 一样完全精确：三角剖分的「外部」用一个单一的符号化幽灵顶点（没有坐标）来表示，而不是一个合成的外包三角形，因此不存在需要权衡处理的尺度依赖问题——已验证在跨度为 `10.0` 的情况下，垂直方向点簇间距小至 `1e-200` 时仍然正确。共圆点意味着多个有效三角剖分之间真正存在平局，而不是只有唯一的「正确」答案；确定性的平局打破规则记录在 [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md) 中，与其他所有退化情形（共线的边界点、恰好位于已有边上的点）一起列出。**已知问题（0.7.0，尚未修复）：** 当 3 个输入点的坐标量级极端且差异巨大时（例如某个坐标接近 `4e304`），可能会 panic —— 在这种极端量级下，`orient2d` 自身可能对参数顺序返回不一致的结果，破坏了本函数自身退化输入处理所依赖的反对称性假设。详见 `CHANGELOG.md` 的 0.7.0 条目和 `tasks/todo.md`。
* `Triangulation2` 的邻接结构 —— `VertexId`/`EdgeId`/`FaceId`，以及 `vertices`/`edges`/`faces`/`edge_vertices`/`adjacent_faces`/`face_vertices`/`neighboring_faces`/`boundary_edges`。这是索引化三角形邻接结构的**静态、构造完成后的快照**（依据 ADR-006 的比较，不具备半边/四边结构的通用性）。`triangles()` 保持其原有的仅坐标契约不变，新增方法完全是附加性的。
* `constrained_delaunay2` / `ConstrainedTriangulation2` —— 2D 约束 Delaunay 三角剖分，有意保持较窄的范围（Phase 6C）：仅支持在*已有*输入顶点之间的不相交约束边，没有自动的交点/Steiner 点生成，也没有细化（refinement）。完全通过翻转已有的 Delaunay 边来构建，使用的是本 crate 自身的 `orient2d`/`incircle`/`segment_intersection_kind` 谓词——ADR-004 的 Phase 6 重新评估预测 CDT **不需要任何新的构造**，实现也证实了这一点：没有构造出任何一个新坐标。约束恢复和 Delaunay 恢复过程都是有界的（不会出现无界循环）；`CdtError` 会将相交/共线的约束、算法穷尽的情形，以及退化点集(点数少于 3 个，或全部共线)作为带类型的错误报告出来，而不会 panic。
* `triangulate_polygon` —— 简单多边形三角剖分（Phase 6D），构建在 Phase 6C 的 CDT 之上：将多边形的每条边都作为约束，然后（对于非凸输入）通过从一个内部种子面出发的纯拓扑洪水填充（flood fill），丢弃多边形外部的凹陷区域面——绝不使用诸如质心之类的构造坐标。无 Steiner 点（每个输出顶点都是多边形自身的顶点之一），自相交的输入会被作为带类型的 `PolygonTriangulationError` 拒绝，同时接受 CCW 和 CW 方向的输入，且结果具有确定性。完整的范围说明表见 [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)，其中还包含了使用 `Triangulation2::validate_topology()` 检查结果时需要注意的事项（该函数的欧拉示性数检查假设三角剖分覆盖了整个凸包，而非凸多边形的三角剖分则有意不满足这一点）。
* `triangulate_polygon_with_holes` —— 带孔多边形三角剖分，是对 `triangulate_polygon` 自身算法的推广而非另起炉灶：孔的边界只是同一个洪水填充算法会在其处停止的更多约束边而已。一个孔嵌套在另一个孔内部的情形超出范围（作为带类型错误处理，而非部分支持）。其他被拒绝的输入（孔位于边界之外、与边界接触或相交、与其他孔接触或相交）同样作为带类型的 `PolygonTriangulationError` 处理，绝不会 panic。`Polygon2::relation_to`/`PointPolygonRelation`（与此同时新增的精确点与多边形谓词）支撑着孔包含关系的检查。
* `Voronoi2` / `voronoi2` —— Voronoi 图，是已有 `Triangulation2` 的对偶：在拓扑（0.5.0）之上具备顶点/边几何（0.7.0）。裁剪和最近邻查询仍未实现，有意推迟。上文提到的 Delaunay 自身的共圆平局规则可能把一个共圆的点簇拆分到多个三角形中；`voronoi2` 通过以 `incircle(...) == Sign::Zero` 为键的并查集（union-find）合并受影响的面，使得这个任意的选择不会作为多余的 Voronoi 顶点或边泄漏出来——通过将同一个共圆点集输入到多个不同的三角剖分中，并验证得到的不仅是同构、而是完全相同的输出，对此进行了验证。拓扑查询 API：`cells`/`vertices`/`edges`、`cell_site`、`neighboring_cells`、`cell_is_unbounded`、`edge_cells`、`edge_kind`、`dual_delaunay_edge`、`vertex_delaunay_faces`，以及 `cell_edges`——按逆时针顺序遍历一个 cell 边界的有序游走（对于 bounded/内部站点的 cell 是一个闭合循环，对于 unbounded/hull 站点的 cell 则是两条射线之间的线性序列），完全基于 `Triangulation2` 已有的面邻接结构构建，没有引入新的数据模型。参见 [`docs/adr/ADR-007-voronoi-diagram-topology.md`](docs/adr/ADR-007-voronoi-diagram-topology.md)。

  `vertex_point`/`edge_geometry`（0.7.0）在此之上加入了实际坐标：`vertex_point` 是某个 Voronoi 顶点所合并的 Delaunay 面组的、经过正确舍入（ADR-004 方式）的外心——对于一个共圆合并组，其中每一个成员面都共享同一个真实外心，因此结果在数学上必然与究竟是哪个成员面计算出来的无关，实际使用的成员面由一个按站点身份排序的规范规则选出，而不是由构造顺序决定。`edge_geometry` 返回一个 bounded 的 `Segment`，或一个 unbounded 的 `Ray`（一个未归一化的外向方向向量——本 crate 中任何地方都没有 `sqrt`/归一化操作），对于任意两个不同的有限 Delaunay 顶点都保证有限且非零，即便坐标是异号且接近 `f64::MAX` 的情形也是如此。当某个面的真实外心不可表示时返回 `Err(VoronoiGeometryError::NonFiniteCircumcenter)`（与 `line_intersection` 不同，这种情况无法通过重新缩放解决——这种溢出是由三角形的长宽比而非坐标量级导致的）；对于本 crate 自身构造过程绝不会违反的内部不变量，则返回 `Err(InvalidTopology)`。两种情况都不会 panic。参见 [`docs/adr/ADR-009-voronoi-geometry.md`](docs/adr/ADR-009-voronoi-geometry.md)。
* `Triangulation2::locate` / `PointLocation` —— 点定位（0.6.0）：`PointLocation::{Vertex(VertexId), Edge(EdgeId), Face(FaceId), Outside}`，一个封闭的枚举（不是 `#[non_exhaustive]`——这 4 个变体恰好是 `Triangulation2` 自身已经封闭的 id 词汇表的闭包，再加上必要的“未命中”情形）。`Outside` 的含义是“不被任何面覆盖”，而不是“在凸包之外”：对于 `triangulate_polygon_with_holes` 的结果，位于孔洞内部的点同样是 `Outside`，因为没有任何面覆盖它。复杂度为 `O(F)`——对每个面进行线性扫描，而不是空间索引；性能有意不属于本次发布的公开契约，因此以后可以在不改变函数签名的情况下换成更快的定位算法。绝不 panic，包括对空的三角剖分。已经针对一个独立的 BigRational oracle 进行了验证，该 oracle 直接检验跨越所有面的聚合/分派逻辑本身，而不仅仅是本 crate 自己的 `Triangle2::relation_to`/`Segment2::relation_to`。参见 [`docs/adr/ADR-008-point-location.md`](docs/adr/ADR-008-point-location.md)。

以上四个谓词共同完成了 v0.1 的健壮谓词范围；上述的基本图元、相交判定、多边形与凸包、Delaunay 三角剖分完成了 Phase 2 到 Phase 4。`segment_intersection` 的 `Proper` 相交点构造（见下文）完成了 Phase 5，而上述的邻接结构、约束 Delaunay 三角剖分与简单多边形三角剖分完成了 Phase 6A-6D。Voronoi 图的*拓扑*（0.5.0）与顶点/边*几何*（0.7.0）、以及点定位（0.6.0）均已实现（见上文）；此后的内容——多边形布尔运算、空间索引/walking locator、最近邻查询，以及尤其是 Voronoi 裁剪——留待以后实现——见[路线图](#roadmap)。

* `predicates::line_intersection`（在 `segment_intersection` 的 `Proper` 情形中被内部调用）—— 本 crate 中首个精确/经过认证的**构造**（依据 ADR-004）。返回最接近真实线—线交点坐标、经过正确舍入（在恰好为平局时采用就近偶数舍入）的 `f64`，而不是一个近似值——这将 IEEE-754 对单次算术运算所作的保证，扩展到了整个几何构造过程。`Point2` 仍然是一个普通的 `f64` 数对；没有引入新的公开类型，也没有引入新的依赖。已针对一个独立的 `BigRational`「这是不是正确舍入后最接近的 `f64`」预言机进行验证，覆盖了不同的量级尺度、混合量级输入，以及一次经验性的下界扫描——见 [`docs/numerical-model.md`](docs/numerical-model.md)。

## <a id="exact-predicates-vs-exact-constructions"></a>精确谓词 vs 精确构造

Kika 的谓词（如 `orient2d`）保证给出数学上正确的**符号**。而保证生成的*坐标*本身也是精确的，是另一个独立的问题（「构造」）——截至 Phase 5，其中一种情形已经得到解决：`segment_intersection` 的 `Proper` 相交点现在是一个经过正确舍入的构造（`predicates::line_intersection`，见 ADR-004），而不再是以前那种朴素的 `f64` 插值。它与该函数可能返回的其他情形（`EndpointTouch`/`CollinearTouch`/`CollinearOverlap`）并列，后者通过直接复用原始输入坐标而早已是精确的。详见 [`docs/architecture.md`](docs/architecture.md) §4.2 与 ADR-004。请不要假设在后续阶段（Phase 6：约束 Delaunay、多边形布尔运算）中实现的构造，在其自身文档明确说明之前，就具有相同的精确性保证。

## 退化情形

共线/共面/共圆/共球的点、重复的点、带符号的零，以及非规范化（subnormal）坐标都被显式处理并经过测试，而不是被当作「足够罕见，可以忽略」的情形来对待。见 [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)。

## 最小示例

```rust
use kika::{Point2, orient2d, Orientation};

let a = Point2::new(0.0, 0.0).unwrap();
let b = Point2::new(1.0, 0.0).unwrap();
let c = Point2::new(0.0, 1.0).unwrap();

assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
```

约束 Delaunay 三角剖分——每一条约束边都保证出现在结果中，即使在通常情况下将其翻转掉才是 Delaunay 三角剖分的选择（这个示例本身是一个 doctest —— `cargo test --doc` —— 作为[`constrained_delaunay2` 自身文档中的示例](src/triangulation/cdt.rs)存在）：

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

简单多边形三角剖分——无 Steiner 点，构建在上述约束 Delaunay 之上（同样是一个 doctest，作为[`triangulate_polygon` 自身文档中的示例](src/triangulation/polygon.rs)存在）。`triangulate_polygon` 只处理单一边界环（无孔洞）；`triangulate_polygon_with_holes` 将同一算法扩展到边界环加零个或多个孔洞环（仍然无 Steiner 点，也不涉及新的构造）：

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

Voronoi 图拓扑——`Triangulation2` 的对偶（同样是一个 doctest，作为[`voronoi2` 自身文档中的示例](src/triangulation/voronoi.rs)存在）：

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

Voronoi 图几何——在上述拓扑之上加入实际坐标（同样是一个 doctest，作为[`vertex_point` 自身文档中的示例](src/triangulation/voronoi.rs)存在）：

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

点定位——`O(F)`，没有空间索引（同样是一个 doctest，作为[`locate` 自身文档中的示例](src/triangulation/locate.rs)存在）：

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

更多可以通过 `cargo run --example <name>` 运行的示例，位于 [`examples/`](examples/) 目录下：

* [`orient2d`](examples/orient2d.rs) —— 基本的转向判定谓词
* [`segment_intersection`](examples/segment_intersection.rs) —— 线段相交的分类与构造
* [`convex_hull`](examples/convex_hull.rs) —— `ExtremesOnly` 与 `KeepAllOnBoundary` 的对比
* [`delaunay`](examples/delaunay.rs) —— 2D Delaunay 三角剖分
* [`polygon_validity`](examples/polygon_validity.rs) —— `basic_validity` 与 `find_self_intersection`
* [`constrained_delaunay`](examples/constrained_delaunay.rs) —— 强制保留某条（可能非 Delaunay 的）指定边
* [`polygon_triangulation`](examples/polygon_triangulation.rs) —— 非凸多边形，附带三角形数量/CCW/面积的检查
* [`polygon_triangulation_with_holes`](examples/polygon_triangulation_with_holes.rs) —— 挖去两个独立孔洞的边界
* [`voronoi`](examples/voronoi.rs) —— 一个共圆的正方形加一个偏离中心的内部点，bounded 与 unbounded 的 cell，以及（0.7.0）每条 cell 边界的实际 segment/ray 几何
* [`locate`](examples/locate.rs) —— vertex/edge/face/outside 的分类，包括孔洞的内部与边界

## WASM

谓词核心不包含任何操作系统相关或平台相关的代码，可以为 `wasm32-unknown-unknown` 构建；这一点已在 CI 中验证。目前还不存在任何 WASM 专用绑定（如 `wasm-bindgen` 等）。

## 与 CGAL 的区别

Kika 不链接 CGAL，也不与其共享任何源代码。CGAL 的*计划*用途仅仅是作为开发过程中一个独立的外部差分测试预言机（见项目开发说明文档 §10）——绝不会成为 `kika` crate 本身的运行时或构建依赖——不过该对照程序目前尚不存在。详见上文的[为什么不直接使用 CGAL？](#why-not-just-use-cgal)。

## 稳定性

Pre-1.0 阶段，没有 semver 保证。某些计算几何库（包括 CGAL）中出现的那种公开 `Kernel` trait 设计，本项目有意尚未确定下来——见 ADR-004。截至 0.3.0，公开的 `Result` 风格错误枚举（`KikaError`、`CdtError`、`PolygonTriangulationError`，以及自 0.7.0 起的 `VoronoiGeometryError`）已标记为 `#[non_exhaustive]`，因此未来新增 variant 不会破坏调用方已带通配符分支的 `match`——详见 `CHANGELOG.md`。

## <a id="maturity"></a>成熟度

| 功能 | 状态 |
|---|---|
| 谓词（`orient2d`、`orient3d`、`incircle`、`insphere`） | 足够稳定、可供评估使用 —— 过滤器 + 精确回退，已针对独立预言机验证 |
| 线段相交判定 | 已实现 —— 分类精确，`Proper` 构造经过正确舍入（ADR-004） |
| 凸包 | 已实现 —— 完全精确 |
| Delaunay 三角剖分 | 已实现 —— 完全精确，无合成坐标 |
| 三角剖分邻接关系（顶点/边/面查询） | 已实现 —— `VertexId`/`EdgeId`/`FaceId`，邻接/边界查询，内部拓扑校验器（ADR-006） |
| 约束 Delaunay | 已实现 —— 范围有限：仅支持已有顶点之间的不相交约束，无 Steiner 点（Phase 6C） |
| 简单多边形三角剖分 | 已实现 —— 无 Steiner 点，自相交输入会被拒绝（Phase 6D）。支持孔洞（0.4.0，`triangulate_polygon_with_holes`）—— 嵌套孔洞超出范围，作为带类型错误处理 |
| Voronoi 图 | 已实现 —— 拓扑（0.5.0）：cells/vertices/edges，有序的 `cell_edges()` 边界游走；顶点/边几何（0.7.0）：`vertex_point`/`edge_geometry`，经正确舍入的外心，未归一化的 ray 方向；尚无裁剪或最近邻查询 |
| 点定位 | 已实现 —— `Triangulation2::locate`（0.6.0），`O(F)` 线性扫描，已针对独立的 BigRational oracle 验证；尚无空间索引/walking locator 或最近邻查询 |
| 多边形布尔运算 | 未实现 —— 精确性模型仍未确定，见 ADR-004 |
| 3D 网格运算 | 未实现 |

## 许可证

本项目基于以下任一许可证授权

* MIT 许可证（[LICENSE-MIT](LICENSE-MIT)）
* Apache 许可证 2.0 版（[LICENSE-APACHE](LICENSE-APACHE)）

具体选择哪一种由使用者自行决定。

## <a id="roadmap"></a>路线图

Phase 1（健壮谓词）、Phase 2（2D 基本图元与相交判定）、Phase 3（2D 凸包）、Phase 4（2D Delaunay 三角剖分）、Phase 5（经认证/精确的构造 —— 精确的 `Proper` 线段交点）、Phase 6A-6D（三角剖分邻接结构、范围有限的约束 Delaunay、范围有限的简单多边形三角剖分）、Voronoi 图的*拓扑*（0.5.0）与顶点/边*几何*（0.7.0），以及点定位（0.6.0）均已完成。尚未实现的内容：Voronoi 裁剪、最近邻查询；`locate` 的空间索引/walking locator；多边形/网格布尔运算；顶点删除；Delaunay 细化（refinement）；网格修复；曲面重建；点云处理。分阶段的待办事项列表见 [`tasks/todo.md`](tasks/todo.md)，在 `crates.io`/GitHub 发布前已验证的内容见 [`docs/release-checklist.md`](docs/release-checklist.md)（0.2.0 到 0.7.0 均已发布——见 `CHANGELOG.md`）。
