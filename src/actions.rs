/// Actions that can be taken in response to user input (eg. click on the UI, pressing a shortcut)
/// This enum only captures the actions that can happen in the domaing of rendering
/// For example simple camera movement actions are handled directly
pub enum RenderingAction {
    GenerateCubeMapFromEquirectangular,
    BakeDiffuseIrradianceMap,
    SaveDiffuseIrradianceMapToFile,
}
