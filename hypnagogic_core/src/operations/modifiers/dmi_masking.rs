use dmi::icon::Icon;
use dmi::icon::IconState;
use image::Rgba;
use image::{DynamicImage, GenericImage, GenericImageView, Pixel};
use serde::{Deserialize, Serialize};
use tracing::debug;
use std::cmp::max;
use crate::operations::error::{ProcessorError, ProcessorResult};
use crate::operations::modifiers::error::{InconsistentDelays, InconsistentDirectional, MaskingError};
use crate::operations::{IconOperationConfig, InputIcon, OperationMode, ProcessorPayload};

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct DMIMasking {
    // List of icon states to mask
    pub target_states: Vec<String>,
    // Suffix to use for masked states
    pub mask_suffix: String,
    // Suffix to use for inverse masked states
    pub unmasked_suffix: String,
}

impl IconOperationConfig for DMIMasking {
    #[tracing::instrument(skip(input))]
    fn perform_operation(
        &self,
        input: &InputIcon,
        mode: OperationMode,
    ) -> ProcessorResult<ProcessorPayload> {
        debug!("Starting dmi masking");
        let InputIcon::Dmi(icon) = input else {
            return Err(ProcessorError::DMINotFound);
        };

        // First, pull out icon states from DMI
        let working_states = icon.states.clone();
        let states_iter = working_states.clone().into_iter();
      
        let extract_state = |extract: &String| -> Option<usize> {
            states_iter.clone().position(|state| state.name == *extract)
        };

        let mut missing_states: Vec<String> = vec![];
        let paired_states = self.target_states.clone().into_iter()
            .map(|name| (name.clone(), extract_state(&name))).zip(self.target_states.clone().into_iter()
            .map(|name| { 
                let mask_name = format!("{name}_mask");
                (mask_name.clone(), extract_state(&mask_name))
            }
            )).filter_map(|((base_name, base_index), (mask_name, mask_index))| {
                let mut failed = false;
                if base_index.is_none() {
                    missing_states.push(base_name);
                    failed = true;
                }
                if mask_index.is_none() {
                    missing_states.push(mask_name);
                    failed = true;
                }
                if failed {
                    return None
                }

                let base_state = working_states.get(base_index.unwrap()).unwrap();
                let mask_state = working_states.get(mask_index.unwrap()).unwrap();
                Some((base_state, mask_state, max(base_index.unwrap(), mask_index.unwrap())))

            }).collect::<Vec<(&IconState, &IconState, usize)>>();
        if missing_states.len() > 0 {
            return Err(ProcessorError::from(MaskingError::NonexistantStates(missing_states.join(", "))))
        }

        let mut mismatched_dirs: Vec<InconsistentDirectional> = vec![];
        let mut mismatched_delays: Vec<InconsistentDelays> = vec![];
        for (base, mask, _) in paired_states.clone() {
            if base.dirs != mask.dirs {
                mismatched_dirs.push(InconsistentDirectional {target_name: base.name.clone(), mask_name: mask.name.clone()});
            }
            if base.delay != mask.delay {
                mismatched_delays.push(InconsistentDelays {target_name: base.name.clone(), target_delays: base.delay.clone().unwrap_or_default(), mask_name: mask.name.clone(), mask_delays: mask.delay.clone().unwrap_or_default()});
            }
        }
        
        if mismatched_dirs.len() > 0 {
            return Err(ProcessorError::from(MaskingError::MismatchedDirectionals(mismatched_dirs)))
        }
        if mismatched_delays.len() > 0 {
            return Err(ProcessorError::from(MaskingError::MismatchedDelays(mismatched_delays)))
        }

        let mut final_states = icon.states.clone();
            // Actually do the masking
        for (base_state, mask_state, greater_index) in paired_states {
            let mut masked_frames = vec![];
            let mut unmasked_frames = vec![];
            
            for (base, mask) in base_state.images.clone().into_iter().zip(mask_state.images.clone().into_iter()) {
                // Alright, now we just need to mask our image with the alpha channel of the mask
                let mut masked_frame = DynamicImage::new_rgba8(icon.width, icon.height);
                masked_frame.copy_from(&base, 0, 0)?;
                mask.pixels().for_each(|(x, y, _pixel)| {
                    let alpha = mask.get_pixel(x, y).to_rgba()[3];
                    if alpha != 0 {
                        masked_frame.put_pixel(x, y, Rgba([0,0,0,0]))
                    }
                });
                masked_frames.push(masked_frame);
        
                // And the inverse
                let mut unmasked_frame = DynamicImage::new_rgba8(icon.width, icon.height);
                unmasked_frame.copy_from(&base, 0, 0)?;
                mask.pixels().for_each(|(x, y, _pixel)| {
                    let alpha = mask.get_pixel(x, y).to_rgba()[3];
                    if alpha == 0 {
                        unmasked_frame.put_pixel(x, y, Rgba([0,0,0,0]))
                    }
                });
                unmasked_frames.push(unmasked_frame);
            }
            
            // Then we write our states and drop any with a matching name
            let masked_name = format!("{}_{}", base_state.name, self.mask_suffix);
            let possible_mask = extract_state(&masked_name);
            let masked_state = IconState {
                name: masked_name,
                images: masked_frames,
                ..base_state.clone()
            };
            if let Some(existing_masked_index) = possible_mask {
                final_states[existing_masked_index] = masked_state;
            } 
            else {
                if greater_index + 1 < final_states.len(){
                    final_states.insert(greater_index + 1, masked_state);
                }
                else {
                    final_states.push(masked_state)
                }
            }
            
            let unmasked_name = format!("{}_{}", base_state.name, self.unmasked_suffix);
            let possible_unmasked = extract_state(&unmasked_name);
            let unmasked_state = IconState {
                name: unmasked_name,
                images: unmasked_frames,
                ..base_state.clone()
            };
            if let Some(existing_unmasked_index) = possible_unmasked {
                final_states[existing_unmasked_index] = unmasked_state;
            } 
            else {
                if greater_index + 2 < final_states.len(){
                    final_states.insert(greater_index + 2, unmasked_state);
                }
                else {
                    final_states.push(unmasked_state)
                }
            }
        }

        Ok(ProcessorPayload::from_icon(Icon {
            states: final_states,
            ..icon.clone()
        }))
    }

    fn verify_config(&self) -> ProcessorResult<()> {
        // TODO: Actual verification
        Ok(())
    }
}
