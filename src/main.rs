use eframe;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use gigachat::TemplateApp;

  let native_options = eframe::NativeOptions { 
    transparent: true, 
    decorated: true,
    ..Default::default() 
  };
  eframe::run_native("Gigachat 0.0", native_options, Box::new(|cc| Box::new(TemplateApp::new(cc))));
}
