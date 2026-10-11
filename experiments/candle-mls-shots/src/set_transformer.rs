//! Tiny permutation-equivariant player set-attention encoder.
//!
//! Not trained on the 2026 MLS pilot: its dated dataset lacks complete
//! pre-match team/opponent candidate sets. Observed starting XIs would leak.

use candle_core::{DType, Device, Result, Tensor, Var};
use candle_nn::ops::softmax;

fn token_linear(tokens: &Tensor, weight: &Tensor) -> Result<Tensor> {
    let (batch, tokens_count, input_width) = tokens.dims3()?;
    let (matrix_input, output_width) = weight.dims2()?;
    if input_width != matrix_input {
        return Err(candle_core::Error::Msg(
            "set-attention linear dimension mismatch".into(),
        ));
    }
    tokens
        .reshape((batch * tokens_count, input_width))?
        .matmul(weight)?
        .reshape((batch, tokens_count, output_width))
}

/// One attention head with no positional encoding.
/// Inputs have shape [fixture, candidate, feature]; a mask value of 1 is valid.
pub struct PlayerSetTransformer {
    width: usize,
    w_input: Var,
    w_q: Var,
    w_k: Var,
    w_v: Var,
    w_output: Var,
}

impl PlayerSetTransformer {
    pub fn new(features: usize, width: usize, device: &Device) -> Result<Self> {
        assert!(features > 0 && width > 0);
        let deterministic = |n: usize, step: f64| -> Vec<f32> {
            (0..n)
                .map(|i| (0.08 * (step * (i + 1) as f64).sin()) as f32)
                .collect()
        };
        let matrix = |rows: usize, cols: usize, step: f64| -> Result<Var> {
            Var::from_tensor(&Tensor::from_vec(
                deterministic(rows * cols, step),
                (rows, cols),
                device,
            )?)
        };
        Ok(Self {
            width,
            w_input: matrix(features, width, 0.27)?,
            w_q: matrix(width, width, 0.19)?,
            w_k: matrix(width, width, 0.41)?,
            w_v: matrix(width, width, 0.73)?,
            w_output: matrix(width, 1, 1.31)?,
        })
    }

    pub fn parameters(&self) -> Vec<Var> {
        vec![
            self.w_input.clone(),
            self.w_q.clone(),
            self.w_k.clone(),
            self.w_v.clone(),
            self.w_output.clone(),
        ]
    }

    pub fn forward(&self, players: &Tensor, mask: &Tensor) -> Result<Tensor> {
        let (batches, player_count, _) = players.dims3()?;
        if mask.dims() != [batches, player_count] {
            return Err(candle_core::Error::Msg(
                "player-mask dimensions do not match".into(),
            ));
        }
        if mask.dtype() != DType::F32 || players.dtype() != DType::F32 {
            return Err(candle_core::Error::Msg("F32 tensors required".into()));
        }
        let valid = mask.to_vec2::<f32>()?;
        if valid
            .iter()
            .any(|row| row.iter().all(|v| *v == 0.0) || row.iter().any(|v| *v != 0.0 && *v != 1.0))
        {
            return Err(candle_core::Error::Msg(
                "mask must contain 0/1 values and a real player in each fixture".into(),
            ));
        }
        let x = token_linear(players, self.w_input.as_tensor())?.relu()?;
        let q = token_linear(&x, self.w_q.as_tensor())?;
        let k = token_linear(&x, self.w_k.as_tensor())?;
        let v = token_linear(&x, self.w_v.as_tensor())?;
        let logits = q
            .matmul(&k.transpose(1, 2)?)?
            .affine(1.0 / (self.width as f64).sqrt(), 0.0)?;
        let key_offset = mask.unsqueeze(1)?.affine(1e9, -1e9)?;
        let attention = softmax(&logits.broadcast_add(&key_offset)?, 2)?;
        let context = attention.matmul(&v)?;
        let residual_hidden = (x + context)?.relu()?;
        let residual = token_linear(&residual_hidden, self.w_output.as_tensor())?
            .tanh()?
            .affine(1.2, 0.0)?;
        residual.broadcast_mul(&mask.unsqueeze(2)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(players: &[Vec<f32>], mask: &[f32]) -> Result<(Tensor, Tensor)> {
        let device = Device::Cpu;
        Ok((
            Tensor::from_vec(
                players.iter().flatten().copied().collect::<Vec<_>>(),
                (1, players.len(), players[0].len()),
                &device,
            )?,
            Tensor::from_vec(mask.to_vec(), (1, players.len()), &device)?,
        ))
    }

    #[test]
    fn permutation_equivariance_player_order_has_no_effect() -> Result<()> {
        let model = PlayerSetTransformer::new(3, 8, &Device::Cpu)?;
        let players = vec![
            vec![1.0, 0.1, 0.3],
            vec![0.1, -0.4, 0.8],
            vec![-1.0, 2.0, 0.0],
        ];
        let (features, mask) = fixture(&players, &[1.0; 3])?;
        let reference = model
            .forward(&features, &mask)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let order = [2, 0, 1];
        let shuffled = order.map(|index| players[index].clone()).to_vec();
        let (features, mask) = fixture(&shuffled, &[1.0; 3])?;
        let changed = model
            .forward(&features, &mask)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        for (i, j) in order.iter().enumerate() {
            assert!((changed[i] - reference[*j]).abs() < 1e-6);
        }
        Ok(())
    }

    #[test]
    fn masked_padding_does_not_change_valid_player_outputs() -> Result<()> {
        let model = PlayerSetTransformer::new(3, 8, &Device::Cpu)?;
        let first = vec![0.1, 0.3, 0.8];
        let second = vec![0.8, 0.4, -0.1];
        let (features, mask) = fixture(&[first.clone(), second.clone()], &[1.0, 1.0])?;
        let before = model
            .forward(&features, &mask)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let (features, mask) = fixture(
            &[first, second, vec![999.0, -999.0, 100.0]],
            &[1.0, 1.0, 0.0],
        )?;
        let after = model
            .forward(&features, &mask)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        assert!((before[0] - after[0]).abs() < 1e-6);
        assert!((before[1] - after[1]).abs() < 1e-6);
        assert_eq!(after[2], 0.0);
        Ok(())
    }

    #[test]
    fn attention_parameters_support_autodiff() -> Result<()> {
        let model = PlayerSetTransformer::new(3, 8, &Device::Cpu)?;
        let (features, mask) = fixture(&[vec![1.0, 0.1, 0.3], vec![0.1, -0.4, 0.8]], &[1.0, 1.0])?;
        let loss = model.forward(&features, &mask)?.sqr()?.mean_all()?;
        let gradients = loss.backward()?;
        let mut nonzero = 0;
        for parameter in model.parameters() {
            let grad = gradients
                .get(&parameter)
                .expect("attention parameter must receive gradient");
            for value in grad.flatten_all()?.to_vec1::<f32>()? {
                assert!(value.is_finite());
                nonzero += usize::from(value.abs() > 1e-12);
            }
        }
        assert!(nonzero > 0);
        Ok(())
    }
}
