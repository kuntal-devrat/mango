use super::length::Length;

/// A single CSS 2D/3D transform function.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformFunction {
    /// `translateX(tx)` / `translateY(ty)` / `translate(tx, ty)`
    Translate(f32, f32),
    /// `translateX(tx)`
    TranslateX(f32),
    /// `translateY(ty)`
    TranslateY(f32),
    /// `translate(tx, ty)` with Length units (supports px, %, em, etc.)
    TranslateLen(Length, Length),
    /// `translateX(tx)` with Length units
    TranslateXLen(Length),
    /// `translateY(ty)` with Length units
    TranslateYLen(Length),
    /// `translateZ(tz)` (3D)
    TranslateZ(f32),
    /// `translate3d(tx, ty, tz)` (3D)
    Translate3d(f32, f32, f32),
    /// `rotate(angle_deg)`
    Rotate(f32),
    /// `rotateX(angle_deg)` (3D)
    RotateX(f32),
    /// `rotateY(angle_deg)` (3D)
    RotateY(f32),
    /// `rotateZ(angle_deg)` (3D - equivalent to 2D rotate)
    RotateZ(f32),
    /// `rotate3d(x, y, z, angle_deg)` (3D)
    Rotate3d(f32, f32, f32, f32),
    /// `scaleX(sx)` / `scaleY(sy)` / `scale(sx, sy)`
    Scale(f32, f32),
    /// `scaleX(sx)`
    ScaleX(f32),
    /// `scaleY(sy)`
    ScaleY(f32),
    /// `scaleZ(sz)` (3D)
    ScaleZ(f32),
    /// `scale3d(sx, sy, sz)` (3D)
    Scale3d(f32, f32, f32),
    /// `skewX(angle_deg)` / `skewY(angle_deg)` / `skew(ax, ay)`
    Skew(f32, f32),
    /// `matrix(a, b, c, d, e, f)`
    Matrix(f32, f32, f32, f32, f32, f32),
    /// `matrix3d(m11, m12, ... m44)` (3D)
    Matrix3d([f32; 16]),
    /// `perspective(d)` — 3D perspective
    Perspective(f32),
}

/// Resolved CSS transform: a list of transform functions applied left to right.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transform(pub Vec<TransformFunction>);

impl Transform {
    pub fn is_identity(&self) -> bool {
        self.0.is_empty()
    }

    /// Collapses the transform list into a 2D affine matrix [a, b, c, d, e, f]
    /// (same as SVG/Canvas `matrix(a, b, c, d, e, f)`), projecting 3D functions down per CSS Transforms 2.
    pub fn to_matrix(&self) -> [f32; 6] {
        self.to_matrix_with_size(0.0, 0.0)
    }

    /// Collapses the transform list into a 2D affine matrix, resolving percentage translations against box size.
    pub fn to_matrix_with_size(&self, width: f32, height: f32) -> [f32; 6] {
        let mul4 = |a: [f32; 16], b: [f32; 16]| -> [f32; 16] {
            let mut out = [0.0; 16];
            for r in 0..4 {
                for c in 0..4 {
                    out[r * 4 + c] = a[r * 4] * b[c]
                        + a[r * 4 + 1] * b[4 + c]
                        + a[r * 4 + 2] * b[8 + c]
                        + a[r * 4 + 3] * b[12 + c];
                }
            }
            out
        };
        let mut mat4 = [
            1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        for func in &self.0 {
            let n = match func {
                TransformFunction::Translate(tx, ty) => [1.0, 0.0, 0.0, 1.0, *tx, *ty],
                TransformFunction::TranslateX(tx) => [1.0, 0.0, 0.0, 1.0, *tx, 0.0],
                TransformFunction::TranslateY(ty) => [1.0, 0.0, 0.0, 1.0, 0.0, *ty],
                TransformFunction::TranslateLen(tx, ty) => {
                    let rx = match tx {
                        Length::Px(px) => *px,
                        Length::Percent(p) => width * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    let ry = match ty {
                        Length::Px(px) => *px,
                        Length::Percent(p) => height * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    [1.0, 0.0, 0.0, 1.0, rx, ry]
                }
                TransformFunction::TranslateXLen(tx) => {
                    let rx = match tx {
                        Length::Px(px) => *px,
                        Length::Percent(p) => width * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    [1.0, 0.0, 0.0, 1.0, rx, 0.0]
                }
                TransformFunction::TranslateYLen(ty) => {
                    let ry = match ty {
                        Length::Px(px) => *px,
                        Length::Percent(p) => height * p / 100.0,
                        Length::Em(em) => em * 16.0,
                        Length::Rem(rem) => rem * 16.0,
                        _ => 0.0,
                    };
                    [1.0, 0.0, 0.0, 1.0, 0.0, ry]
                }
                TransformFunction::TranslateZ(tz) => {
                    // 3D translation: z translation is stored for perspective calculation
                    let mat = [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, *tz,
                        1.0,
                    ];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Translate3d(tx, ty, tz) => {
                    let mat = [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, *tx, *ty, *tz,
                        1.0,
                    ];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Rotate(deg) | TransformFunction::RotateZ(deg) => {
                    let r = deg.to_radians();
                    let (s, c) = r.sin_cos();
                    [c, s, -s, c, 0.0, 0.0]
                }
                TransformFunction::RotateX(deg) => {
                    let r = deg.to_radians();
                    let (s, c) = r.sin_cos();
                    let mat = [
                        1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0,
                    ];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::RotateY(deg) => {
                    let r = deg.to_radians();
                    let (s, c) = r.sin_cos();
                    let mat = [
                        c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0,
                    ];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Rotate3d(x, y, z, deg) => {
                    let len = (x * x + y * y + z * z).sqrt();
                    if len < 1e-6 {
                        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
                    } else {
                        let nx = x / len;
                        let ny = y / len;
                        let nz = z / len;
                        let r = deg.to_radians();
                        let (s, c) = r.sin_cos();
                        let omc = 1.0 - c;
                        let a = c + nx * nx * omc;
                        let b = ny * nx * omc + nz * s;
                        let c_elem = nx * ny * omc - nz * s;
                        let d = c + ny * ny * omc;
                        [a, b, c_elem, d, 0.0, 0.0]
                    }
                }
                TransformFunction::Scale(sx, sy) => [*sx, 0.0, 0.0, *sy, 0.0, 0.0],
                TransformFunction::ScaleX(sx) => [*sx, 0.0, 0.0, 1.0, 0.0, 0.0],
                TransformFunction::ScaleY(sy) => [1.0, 0.0, 0.0, *sy, 0.0, 0.0],
                TransformFunction::ScaleZ(sz) => {
                    let mat = [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, *sz, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Scale3d(sx, sy, sz) => {
                    let mat = [
                        *sx, 0.0, 0.0, 0.0, 0.0, *sy, 0.0, 0.0, 0.0, 0.0, *sz, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ];
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Skew(ax, ay) => {
                    let tx = ax.to_radians().tan();
                    let ty = ay.to_radians().tan();
                    [1.0, ty, tx, 1.0, 0.0, 0.0]
                }
                TransformFunction::Matrix(a, b, c, d, e, f) => [*a, *b, *c, *d, *e, *f],
                TransformFunction::Matrix3d(m) => {
                    let mut mat = [0.0f32; 16];
                    for (i, item) in mat.iter_mut().enumerate() {
                        *item = *m.get(i).unwrap_or(if i % 5 == 0 { &1.0 } else { &0.0 });
                    }
                    mat4 = mul4(mat4, mat);
                    continue;
                }
                TransformFunction::Perspective(d) => {
                    if *d > 0.0 {
                        let mat = [
                            1.0,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                            1.0,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                            1.0,
                            -1.0 / *d,
                            0.0,
                            0.0,
                            0.0,
                            1.0,
                        ];
                        mat4 = mul4(mat4, mat);
                    }
                    continue;
                }
            };
            // Embed 2D affine [a, b, c, d, e, f] into 4x4 and multiply
            let mat = [
                n[0], n[1], 0.0, 0.0, n[2], n[3], 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, n[4], n[5], 0.0,
                1.0,
            ];
            mat4 = mul4(mat4, mat);
        }

        // Project 4x4 homogeneous matrix to 2D affine [a, b, c, d, e, f]
        let w = mat4[15];
        let scale = if w.abs() > 1e-6 { 1.0 / w } else { 1.0 };
        [
            mat4[0] * scale,
            mat4[1] * scale,
            mat4[4] * scale,
            mat4[5] * scale,
            mat4[12] * scale,
            mat4[13] * scale,
        ]
    }
}
