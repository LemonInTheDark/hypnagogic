use thiserror::Error;
use user_error::UFE;

use crate::util::delays::text_delays;

#[derive(Debug)]
pub struct InconsistentDirectional {
    pub target_name: String,
    pub mask_name: String,
}

#[derive(Debug)]
pub struct InconsistentDelays {
    pub target_name: String,
    pub target_delays: Vec<f32>,
    pub mask_name: String,
    pub mask_delays: Vec<f32>,
}

#[derive(Debug, Error)]
pub enum MaskingError {
    #[error("Missing Icon States")]
    NonexistantStates(String),
    #[error("Directional Mismatch")]
    MismatchedDirectionals(Vec<InconsistentDirectional>),
    #[error("Delay Mismatch")]
    MismatchedDelays(Vec<InconsistentDelays>),
}

impl UFE for MaskingError {
    fn summary(&self) -> String {
        format!("{self}")
    }

    fn reasons(&self) -> Option<Vec<String>> {
        match self {
            MaskingError::NonexistantStates(reason) => {
                Some(vec![format!(
                    "The following icon states were expected but not found: [{reason}]"
                )])
            }
            MaskingError::MismatchedDirectionals(fuck_ups) => {
                let mut hand_back: Vec<String> = vec![];
                for fuck_up in fuck_ups {
                    hand_back.push(format!("Icon state {}'s direction does not match {}", fuck_up.target_name, fuck_up.mask_name))
                }
                Some(hand_back)
            }
            MaskingError::MismatchedDelays ( problems ) => {
                let mut hand_back: Vec<String> = vec![];
                for problem in problems {
                    hand_back.push(format!(
                        "Icon state {}'s delays {} do not match {}'s {}",
                        problem.target_name,
                        text_delays(&problem.target_delays, "ds"),
                        problem.mask_name,
                        text_delays(&problem.mask_delays, "ds"),
                    ));
                }
                Some(hand_back)
            }
        }
    }

    fn helptext(&self) -> Option<String> {
        match self {
            MaskingError::NonexistantStates(_) => {
                Some("Did you remember to save?".to_string())
            }
            MaskingError::MismatchedDirectionals(_) => {
                Some(
                    "Check if the two icon states have the same amount of directionals. Failing this, did you save?".to_string(),
                )
            }
            MaskingError::MismatchedDelays (_) => {
                Some(
                    "Make sure all the delays line up correctly, careful this can be a bit annoying".to_string(),
                )
            }
        }
    }
}
