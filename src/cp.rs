use itertools::Itertools;
use rusty_machine::linalg::{Matrix, BaseMatrix};
use rusty_machine::learning::LearningResult;

use ncm::NonConformityScorer;


/// A Confidence Predictor (either transductive or inductive CP)
pub trait ConfidencePredictor<T> {
    fn train(&mut self, inputs: &Vec<T>, targets: &Vec<usize>) -> LearningResult<()>;
    fn predict(&mut self, inputs: &Vec<T>) -> LearningResult<Matrix<bool>>;
    fn predict_confidence(&mut self, inputs: &Vec<T>) -> LearningResult<Matrix<f64>>;
    fn set_epsilon(&mut self, epsilon: f64);
    // TODO:
    // fn predict_region(&self, pvalues: &Matrix<f64>, epsilon: f64) -> ...
    // fn update(&self, inputs: &Vec<T>, targets: &Vec<usize>) -> LearningResult<()>;

}

/// Transductive Conformal Predictor
/// 
/// T: type of an object (e.g., Vec<f64>).
pub struct CP<T> {
    ncm: Box<NonConformityScorer<T>>,
    epsilon: Option<f64>,
    smooth: bool,
    /* Training inputs are stored in a train_inputs, indexed
     * by a label y, where train_inputs[y] contains all training
     * inputs with label y.
     */
    train_inputs: Option<Vec<Vec<T>>>,
}

impl<T> CP<T> {
    pub fn new(ncm: Box<NonConformityScorer<T>>, epsilon: Option<f64>, smooth: bool)
            -> CP<T> {
        CP {
            ncm: ncm,
            epsilon: epsilon,
            smooth: smooth,
            train_inputs: None,
        }
    }
}

impl<T> ConfidencePredictor<T> for CP<T> where T: Clone {

    fn set_epsilon(&mut self, epsilon: f64) {
        self.epsilon = Some(epsilon);
    }

    fn train(&mut self, inputs: &Vec<T>, targets: &Vec<usize>)
            -> LearningResult<()> {

        /* Split examples w.r.t. their labels. For each unique label y,
         * self.train_inputs[y] will contain a vector of the inputs with
         * label y.
         */
        let n_labels = targets.iter()
                              .unique()
                              .count();
        self.train_inputs = Some(inputs.iter()
                                       .zip(targets)
                                       .fold(vec![vec![]; n_labels],
                                             |mut res, (x, y)| {
                                                res[*y].push(x.clone());
                                                res
                                             }));

        Ok(())
    }

    /// Returns a region prediction as a matrix of boolean
    /// values, where each column corresponds to a label,
    /// each value to an input object, and the value is
    /// true if the label conforms the distribution, false
    /// otherwise.
    fn predict(&mut self, inputs: &Vec<T>) -> LearningResult<Matrix<bool>> {
        let epsilon = self.epsilon.expect("Specify epsilon to perform a standard predict()");

        let pvalues = self.predict_confidence(inputs).expect("Failed to predict p-values");

        let preds = Matrix::from_fn(pvalues.rows(), pvalues.cols(),
                                    |j, i| pvalues[[i,j]] > epsilon);
        
        Ok(preds)
    }

    /// Returns the p-values corresponding to the labels
    /// for each object provided as input.
    fn predict_confidence(&mut self, inputs: &Vec<T>) -> LearningResult<Matrix<f64>> {

        let error_msg = "You should train the model first";

        let n_labels = self.train_inputs.as_ref()
                                        .expect(error_msg)
                                        .len();

        let n_test = inputs.len();

        let mut pvalues = Matrix::new(n_test, n_labels,
                                      vec![0.0; n_test*n_labels]);

        /* We first iterate through labels and then through input
         * examples.
         */
        for y in 0..n_labels {

            //train_inputs_l.reserve(1);
            let n_tmp = self.train_inputs.as_ref()
                                         .expect(error_msg)[y]
                                         .len() + 1; /* Count includes 1 test example */

            for (i, x) in inputs.iter().enumerate() {
                /* Temporarily add x to the training data with the
                 * current label.
                 */
                {
                    self.train_inputs.as_mut()
                                     .expect(error_msg)[y]
                                     .push(x.clone());
                }

                /* Compute nonconformity scores.
                 */
                let scores = {
                    let train_inputs = self.train_inputs.as_ref()
                                                        .expect(error_msg)[y]
                                                        .as_slice();
                    (0..n_tmp).into_iter()
                              .map(|j| self.ncm.score(j, train_inputs))
                              .collect::<Vec<_>>()
                };

                /* Compute p-value for the current label.
                 */
                let pvalue = if self.smooth {
                    unimplemented!();

                    let r = 0.1;
                    let a = scores.iter()
                                  .filter(|&s| *s > scores[n_tmp-1])
                                  .count() as f64;
                    let b = scores.iter()
                                  .filter(|&s| *s == scores[n_tmp-1])
                                  .count() as f64;
                    (a + r*b) / n_tmp as f64
                } else {
                    scores.iter()
                          .filter(|&s| *s >= scores[n_tmp-1])
                          .count() as f64 / n_tmp as f64
                };

                pvalues[[i,y]] = pvalue;

                /* Remove x from training data. */
                {
                    self.train_inputs.as_mut()
                                     .expect(error_msg)[y]
                                     .pop();
                }
            }
        }

        Ok(pvalues)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ncm::KNN;
    
    #[test]
    fn train() {
        let ncm = KNN::new(2);
        let mut cp = CP::new(Box::new(ncm), Some(0.1), false);

        let train_inputs = vec![vec![0., 0.],
                                vec![1., 0.],
                                vec![0., 1.],
                                vec![1., 1.],
                                vec![2., 2.],
                                vec![1., 2.]];
        let train_targets = vec![0, 0, 1, 1, 2, 2];

        let expected_train_inputs = vec![vec![vec![0., 0.],
                                              vec![1., 0.]],
                                         vec![vec![0., 1.],
                                              vec![1., 1.]],
                                         vec![vec![2., 2.],
                                              vec![1., 2.]]];

        cp.train(&train_inputs, &train_targets).unwrap();

        assert!(cp.train_inputs.unwrap() == expected_train_inputs);
    }
}
