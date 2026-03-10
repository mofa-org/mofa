use super::*;
use serde_json::json;
use std::path::Path;
use tokio::process::Command;



// OCR TOOL : Extract texts from images using tessaract cli 
// tools utilised : tessaract : For this to be utilised user needs to have tesseract cli installed
// This keeps dependencies light 

pub struct OcrTool{
    definition: ToolDefinition,
}


impl Default for OcrTool{
    fn Default()->Self{
        Self::new()

    }


}

impl OcrTool{

    pub fn new()->Self{}


    fn build_command()->Command{}






}

#[async_trait::async_trait]
impl ToolExecutor for OcrTool{
    fn definition(&self)-> &ToolDefinition{
        &self.definition
    }

    async fn execute(&self)-> PluginResult<serde_json::Value>{ Ok(())}


}
