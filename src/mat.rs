use crate::error::RuntimeError;

/// Element-wise addition of two matrices. Both must have the same dimensions.
pub fn mat_add(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, RuntimeError> {
    let rows = a.len();
    if rows == 0 || b.len() != rows {
        return Err(RuntimeError::General {
            msg: "MAT ADD: dimension mismatch".into(),
        });
    }
    let cols = a[0].len();
    for row in a.iter().chain(b.iter()) {
        if row.len() != cols {
            return Err(RuntimeError::General {
                msg: "MAT ADD: dimension mismatch".into(),
            });
        }
    }
    let mut result = vec![vec![0.0; cols]; rows];
    for i in 0..rows {
        for j in 0..cols {
            result[i][j] = a[i][j] + b[i][j];
        }
    }
    Ok(result)
}

/// Element-wise subtraction of two matrices. Both must have the same dimensions.
pub fn mat_sub(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, RuntimeError> {
    let rows = a.len();
    if rows == 0 || b.len() != rows {
        return Err(RuntimeError::General {
            msg: "MAT SUB: dimension mismatch".into(),
        });
    }
    let cols = a[0].len();
    for row in a.iter().chain(b.iter()) {
        if row.len() != cols {
            return Err(RuntimeError::General {
                msg: "MAT SUB: dimension mismatch".into(),
            });
        }
    }
    let mut result = vec![vec![0.0; cols]; rows];
    for i in 0..rows {
        for j in 0..cols {
            result[i][j] = a[i][j] - b[i][j];
        }
    }
    Ok(result)
}

/// Matrix multiplication. A is m×n, B is n×p, result is m×p.
pub fn mat_mul(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, RuntimeError> {
    let m = a.len();
    if m == 0 || b.is_empty() {
        return Err(RuntimeError::General {
            msg: "MAT MUL: empty matrix".into(),
        });
    }
    let n = a[0].len();
    if b.len() != n {
        return Err(RuntimeError::General {
            msg: format!("MAT MUL: dimension mismatch ({}x{} * {}x?)", m, n, b.len()),
        });
    }
    let p = b[0].len();
    let mut result = vec![vec![0.0; p]; m];
    for i in 0..m {
        for j in 0..p {
            let mut sum = 0.0;
            for k in 0..n {
                sum += a[i][k] * b[k][j];
            }
            result[i][j] = sum;
        }
    }
    Ok(result)
}

/// Scalar multiplication: k * A
pub fn mat_scalar_mul(k: f64, a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    a.iter()
        .map(|row| row.iter().map(|&v| k * v).collect())
        .collect()
}

/// Matrix inverse using Gaussian elimination with partial pivoting.
/// Returns (inverse, determinant). Errors if the matrix is singular or not square.
pub fn mat_inv(a: &[Vec<f64>]) -> Result<(Vec<Vec<f64>>, f64), RuntimeError> {
    let n = a.len();
    if n == 0 {
        return Err(RuntimeError::General {
            msg: "MAT INV: empty matrix".into(),
        });
    }
    for row in a {
        if row.len() != n {
            return Err(RuntimeError::General {
                msg: "MAT INV: matrix must be square".into(),
            });
        }
    }

    // Augmented matrix [A | I]
    let mut aug: Vec<Vec<f64>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = Vec::with_capacity(2 * n);
        row.extend_from_slice(&a[i]);
        for j in 0..n {
            row.push(if i == j { 1.0 } else { 0.0 });
        }
        aug.push(row);
    }

    let mut det = 1.0;

    for col in 0..n {
        // Partial pivoting: find the row with the largest absolute value in this column
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..n {
            let val = aug[row][col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        if max_val < 1e-12 {
            return Err(RuntimeError::General {
                msg: "MAT INV: singular matrix".into(),
            });
        }

        if max_row != col {
            aug.swap(col, max_row);
            det = -det;
        }

        let pivot = aug[col][col];
        det *= pivot;

        // Scale pivot row
        for j in 0..(2 * n) {
            aug[col][j] /= pivot;
        }

        // Eliminate column in all other rows
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = aug[row][col];
            for j in 0..(2 * n) {
                aug[row][j] -= factor * aug[col][j];
            }
        }
    }

    // Extract inverse from the right half of the augmented matrix
    let inverse: Vec<Vec<f64>> = aug
        .into_iter()
        .map(|row| row[n..].to_vec())
        .collect();

    Ok((inverse, det))
}

/// Matrix transpose.
pub fn mat_trn(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if a.is_empty() {
        return Vec::new();
    }
    let rows = a.len();
    let cols = a[0].len();
    let mut result = vec![vec![0.0; rows]; cols];
    for i in 0..rows {
        for j in 0..cols {
            result[j][i] = a[i][j];
        }
    }
    result
}

/// Zero matrix.
pub fn mat_zer(rows: usize, cols: usize) -> Vec<Vec<f64>> {
    vec![vec![0.0; cols]; rows]
}

/// Matrix of ones.
pub fn mat_con(rows: usize, cols: usize) -> Vec<Vec<f64>> {
    vec![vec![1.0; cols]; rows]
}

/// Identity matrix (must be square).
pub fn mat_idn(n: usize) -> Vec<Vec<f64>> {
    let mut result = vec![vec![0.0; n]; n];
    for i in 0..n {
        result[i][i] = 1.0;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mat_add() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = mat_add(&a, &b).unwrap();
        assert_eq!(c, vec![vec![6.0, 8.0], vec![10.0, 12.0]]);
    }

    #[test]
    fn test_mat_sub() {
        let a = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let b = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let c = mat_sub(&a, &b).unwrap();
        assert_eq!(c, vec![vec![4.0, 4.0], vec![4.0, 4.0]]);
    }

    #[test]
    fn test_mat_mul() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = mat_mul(&a, &b).unwrap();
        assert_eq!(c, vec![vec![19.0, 22.0], vec![43.0, 50.0]]);
    }

    #[test]
    fn test_mat_scalar_mul() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let c = mat_scalar_mul(3.0, &a);
        assert_eq!(c, vec![vec![3.0, 6.0], vec![9.0, 12.0]]);
    }

    #[test]
    fn test_mat_inv() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let (inv, det) = mat_inv(&a).unwrap();
        assert!((det - (-2.0)).abs() < 1e-10);
        // A * A^-1 should be identity
        let product = mat_mul(&a, &inv).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (product[i][j] - expected).abs() < 1e-10,
                    "product[{}][{}] = {}, expected {}",
                    i,
                    j,
                    product[i][j],
                    expected
                );
            }
        }
    }

    #[test]
    fn test_mat_inv_singular() {
        let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        assert!(mat_inv(&a).is_err());
    }

    #[test]
    fn test_mat_trn() {
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let t = mat_trn(&a);
        assert_eq!(t, vec![vec![1.0, 4.0], vec![2.0, 5.0], vec![3.0, 6.0]]);
    }

    #[test]
    fn test_mat_zer() {
        assert_eq!(mat_zer(2, 3), vec![vec![0.0; 3]; 2]);
    }

    #[test]
    fn test_mat_con() {
        assert_eq!(mat_con(2, 3), vec![vec![1.0; 3]; 2]);
    }

    #[test]
    fn test_mat_idn() {
        let id = mat_idn(3);
        assert_eq!(
            id,
            vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0],
            ]
        );
    }
}
