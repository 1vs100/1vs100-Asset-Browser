use crate::xur::{Class, Prop};

static T_8214E258: [Prop; 12] = [
    Prop { name: "UseActiveUserAvatar", kind: 1, flags: 0, nested: None },
    Prop { name: "RenderView", kind: 3, flags: 8, nested: None },
    Prop { name: "QueueAnimation", kind: 3, flags: 32, nested: None },
    Prop { name: "PlayAnimation", kind: 3, flags: 32, nested: None },
    Prop { name: "UseCustomView", kind: 1, flags: 8, nested: None },
    Prop { name: "CustomViewEyePosition", kind: 7, flags: 0, nested: None },
    Prop { name: "CustomViewFocusPosition", kind: 7, flags: 0, nested: None },
    Prop { name: "CustomViewUpDirection", kind: 7, flags: 0, nested: None },
    Prop { name: "ShowSelfShadow", kind: 1, flags: 8, nested: None },
    Prop { name: "ShowReflection", kind: 1, flags: 8, nested: None },
    Prop { name: "ShowShadow", kind: 1, flags: 8, nested: None },
    Prop { name: "ShadowLightSource", kind: 7, flags: 0, nested: None },
];

static T_8243BB60: [Prop; 1] = [
    Prop { name: "PressKey", kind: 3, flags: 0, nested: None },
];

static T_8243BCC8: [Prop; 1] = [
    Prop { name: "PressKey", kind: 3, flags: 0, nested: None },
];

static T_8243BD80: [Prop; 1] = [
    Prop { name: "PressKey", kind: 3, flags: 0, nested: None },
];

static T_82440450: [Prop; 2] = [
    Prop { name: "ComboBoxStyle", kind: 3, flags: 0, nested: None },
    Prop { name: "MaxVisibleVertItems", kind: 3, flags: 0, nested: None },
];

static T_82440C88: [Prop; 3] = [
    Prop { name: "SizeMode", kind: 3, flags: 0, nested: None },
    Prop { name: "DataAssociation", kind: 3, flags: 0, nested: None },
    Prop { name: "BrushFlags", kind: 3, flags: 0, nested: None },
];

static T_82441950: [Prop; 1] = [
    Prop { name: "GrayAmount", kind: 4, flags: 0, nested: None },
];

static T_82441CE0: [Prop; 2] = [
    Prop { name: "Brightness", kind: 4, flags: 0, nested: None },
    Prop { name: "BlurAmount", kind: 4, flags: 0, nested: None },
];

static T_82441F18: [Prop; 2] = [
    Prop { name: "Brightness", kind: 4, flags: 0, nested: None },
    Prop { name: "BlurAmount", kind: 4, flags: 0, nested: None },
];

static T_82442148: [Prop; 2] = [
    Prop { name: "Brightness", kind: 4, flags: 0, nested: None },
    Prop { name: "BlurAmount", kind: 4, flags: 0, nested: None },
];

static T_82444A48: [Prop; 2] = [
    Prop { name: "StrokeWidth", kind: 4, flags: 0, nested: None },
    Prop { name: "StrokeColor", kind: 6, flags: 0, nested: None },
];

static T_82444CA8: [Prop; 4] = [
    Prop { name: "Radial", kind: 1, flags: 0, nested: None },
    Prop { name: "NumStops", kind: 2, flags: 10, nested: None },
    Prop { name: "StopColor", kind: 6, flags: 1, nested: None },
    Prop { name: "StopPos", kind: 4, flags: 1, nested: None },
];

static T_824451F0: [Prop; 11] = [
    Prop { name: "FillType", kind: 3, flags: 0, nested: None },
    Prop { name: "FillColor", kind: 6, flags: 2, nested: None },
    Prop { name: "TextureFileName", kind: 5, flags: 2, nested: None },
    Prop { name: "Gradient", kind: 9, flags: 2, nested: Some(&T_82444CA8) },
    Prop { name: "Translation", kind: 7, flags: 0, nested: None },
    Prop { name: "Scale", kind: 7, flags: 0, nested: None },
    Prop { name: "Rotation", kind: 4, flags: 0, nested: None },
    Prop { name: "WrapX", kind: 3, flags: 0, nested: None },
    Prop { name: "WrapY", kind: 3, flags: 0, nested: None },
    Prop { name: "BrushFlags", kind: 3, flags: 0, nested: None },
    Prop { name: "TransformVersion", kind: 3, flags: 10, nested: None },
];

static T_82445660: [Prop; 4] = [
    Prop { name: "Stroke", kind: 9, flags: 0, nested: Some(&T_82444A48) },
    Prop { name: "Fill", kind: 9, flags: 0, nested: Some(&T_824451F0) },
    Prop { name: "Closed", kind: 1, flags: 2, nested: None },
    Prop { name: "Points", kind: 10, flags: 2, nested: None },
];

static T_82445C50: [Prop; 1] = [
    Prop { name: "Direction", kind: 3, flags: 0, nested: None },
];

static T_82445DC8: [Prop; 2] = [
    Prop { name: "Direction", kind: 3, flags: 0, nested: None },
    Prop { name: "MinThumbSize", kind: 3, flags: 0, nested: None },
];

static T_82446550: [Prop; 10] = [
    Prop { name: "TextureFileName", kind: 5, flags: 0, nested: None },
    Prop { name: "Translation", kind: 7, flags: 0, nested: None },
    Prop { name: "Scale", kind: 7, flags: 0, nested: None },
    Prop { name: "Rotation", kind: 4, flags: 0, nested: None },
    Prop { name: "ColorFactor", kind: 8, flags: 0, nested: None },
    Prop { name: "SrcBlendFactor", kind: 3, flags: 8, nested: None },
    Prop { name: "DstBlendFactor", kind: 3, flags: 8, nested: None },
    Prop { name: "WrapX", kind: 3, flags: 0, nested: None },
    Prop { name: "WrapY", kind: 3, flags: 0, nested: None },
    Prop { name: "BrushFlags", kind: 3, flags: 0, nested: None },
];

static T_824469C0: [Prop; 1] = [
    Prop { name: "ColorFactor", kind: 8, flags: 0, nested: None },
];

static T_82446AE8: [Prop; 1] = [
    Prop { name: "Threshold", kind: 4, flags: 0, nested: None },
];

static T_82448718: [Prop; 7] = [
    Prop { name: "Text", kind: 5, flags: 0, nested: None },
    Prop { name: "TextColor", kind: 6, flags: 0, nested: None },
    Prop { name: "DropShadowColor", kind: 6, flags: 0, nested: None },
    Prop { name: "PointSize", kind: 4, flags: 12, nested: None },
    Prop { name: "Font", kind: 5, flags: 0, nested: None },
    Prop { name: "TextStyle", kind: 3, flags: 8, nested: None },
    Prop { name: "LineSpacingAdjust", kind: 2, flags: 0, nested: None },
];

static T_82448B18: [Prop; 7] = [
    Prop { name: "DefaultFocus", kind: 5, flags: 0, nested: None },
    Prop { name: "TransFrom", kind: 5, flags: 0, nested: None },
    Prop { name: "TransTo", kind: 5, flags: 0, nested: None },
    Prop { name: "TransBackFrom", kind: 5, flags: 0, nested: None },
    Prop { name: "TransBackTo", kind: 5, flags: 0, nested: None },
    Prop { name: "InterruptTransitions", kind: 3, flags: 8, nested: None },
    Prop { name: "IgnorePresses", kind: 1, flags: 0, nested: None },
];

static T_82448DA0: [Prop; 4] = [
    Prop { name: "PressPath", kind: 5, flags: 0, nested: None },
    Prop { name: "StayVisible", kind: 1, flags: 0, nested: None },
    Prop { name: "SrcTransIndex", kind: 3, flags: 0, nested: None },
    Prop { name: "DestTransIndex", kind: 3, flags: 0, nested: None },
];

static T_82448F20: [Prop; 7] = [
    Prop { name: "TextColor", kind: 6, flags: 0, nested: None },
    Prop { name: "DropShadowColor", kind: 6, flags: 0, nested: None },
    Prop { name: "PointSize", kind: 4, flags: 12, nested: None },
    Prop { name: "Font", kind: 5, flags: 0, nested: None },
    Prop { name: "TextStyle", kind: 3, flags: 8, nested: None },
    Prop { name: "LineSpacingAdjust", kind: 2, flags: 0, nested: None },
    Prop { name: "DataAssociation", kind: 3, flags: 0, nested: None },
];

static T_82449220: [Prop; 4] = [
    Prop { name: "State", kind: 3, flags: 0, nested: None },
    Prop { name: "Loop", kind: 1, flags: 8, nested: None },
    Prop { name: "Finish", kind: 1, flags: 8, nested: None },
    Prop { name: "Volume", kind: 4, flags: 0, nested: None },
];

static T_82449440: [Prop; 9] = [
    Prop { name: "Layout", kind: 3, flags: 0, nested: None },
    Prop { name: "Checkable", kind: 1, flags: 0, nested: None },
    Prop { name: "SelectedSize", kind: 7, flags: 0, nested: None },
    Prop { name: "KeepSizeUnfocused", kind: 1, flags: 0, nested: None },
    Prop { name: "InterItemSpacing", kind: 7, flags: 0, nested: None },
    Prop { name: "SmoothScroll", kind: 1, flags: 0, nested: None },
    Prop { name: "SmoothScrollBaseSpeed", kind: 4, flags: 0, nested: None },
    Prop { name: "SmoothScrollMaxSpeed", kind: 4, flags: 0, nested: None },
    Prop { name: "SmoothScrollAcceleration", kind: 4, flags: 0, nested: None },
];

static T_82449778: [Prop; 1] = [
    Prop { name: "Wrap", kind: 1, flags: 0, nested: None },
];

static T_82449810: [Prop; 3] = [
    Prop { name: "ItemsText", kind: 5, flags: 0, nested: None },
    Prop { name: "ItemsImage", kind: 5, flags: 0, nested: None },
    Prop { name: "ItemsNavPath", kind: 5, flags: 0, nested: None },
];

static T_82449968: [Prop; 3] = [
    Prop { name: "RangeMin", kind: 2, flags: 0, nested: None },
    Prop { name: "RangeMax", kind: 2, flags: 8, nested: None },
    Prop { name: "Value", kind: 2, flags: 0, nested: None },
];

static T_82449AC0: [Prop; 7] = [
    Prop { name: "RangeMin", kind: 2, flags: 0, nested: None },
    Prop { name: "RangeMax", kind: 2, flags: 8, nested: None },
    Prop { name: "Value", kind: 2, flags: 0, nested: None },
    Prop { name: "Step", kind: 2, flags: 0, nested: None },
    Prop { name: "Vertical", kind: 1, flags: 0, nested: None },
    Prop { name: "AccelInc", kind: 2, flags: 0, nested: None },
    Prop { name: "AccelTime", kind: 3, flags: 0, nested: None },
];

static T_82449D80: [Prop; 6] = [
    Prop { name: "TextLimit", kind: 3, flags: 0, nested: None },
    Prop { name: "AllowedChars", kind: 5, flags: 4, nested: None },
    Prop { name: "PasswordChar", kind: 5, flags: 0, nested: None },
    Prop { name: "ReadOnly", kind: 1, flags: 0, nested: None },
    Prop { name: "Multiline", kind: 1, flags: 0, nested: None },
    Prop { name: "SmoothScroll", kind: 1, flags: 0, nested: None },
];

static T_82449FF8: [Prop; 3] = [
    Prop { name: "SizeMode", kind: 3, flags: 0, nested: None },
    Prop { name: "ImagePath", kind: 5, flags: 16, nested: None },
    Prop { name: "BrushFlags", kind: 3, flags: 0, nested: None },
];

static T_8244A158: [Prop; 6] = [
    Prop { name: "TabCount", kind: 3, flags: 0, nested: None },
    Prop { name: "Wrap", kind: 1, flags: 0, nested: None },
    Prop { name: "UserInterrupt", kind: 1, flags: 0, nested: None },
    Prop { name: "VerticalTabs", kind: 1, flags: 0, nested: None },
    Prop { name: "NoAutoHide", kind: 1, flags: 0, nested: None },
    Prop { name: "DefaultTab", kind: 3, flags: 0, nested: None },
];

static T_8244A3C0: [Prop; 6] = [
    Prop { name: "TextureFileName", kind: 5, flags: 0, nested: None },
    Prop { name: "LeftOffset", kind: 3, flags: 8, nested: None },
    Prop { name: "TopOffset", kind: 3, flags: 8, nested: None },
    Prop { name: "RightOffset", kind: 3, flags: 8, nested: None },
    Prop { name: "BottomOffset", kind: 3, flags: 8, nested: None },
    Prop { name: "NoCenter", kind: 1, flags: 8, nested: None },
];

static T_8244A678: [Prop; 1] = [
    Prop { name: "Id", kind: 5, flags: 0, nested: None },
];

static T_8244CE40: [Prop; 17] = [
    Prop { name: "Id", kind: 5, flags: 0, nested: None },
    Prop { name: "Width", kind: 4, flags: 0, nested: None },
    Prop { name: "Height", kind: 4, flags: 0, nested: None },
    Prop { name: "Position", kind: 7, flags: 0, nested: None },
    Prop { name: "Scale", kind: 7, flags: 0, nested: None },
    Prop { name: "Rotation", kind: 8, flags: 0, nested: None },
    Prop { name: "Opacity", kind: 4, flags: 0, nested: None },
    Prop { name: "Anchor", kind: 3, flags: 0, nested: None },
    Prop { name: "Pivot", kind: 7, flags: 0, nested: None },
    Prop { name: "Show", kind: 1, flags: 0, nested: None },
    Prop { name: "BlendMode", kind: 3, flags: 0, nested: None },
    Prop { name: "DisableTimelineRecursion", kind: 1, flags: 0, nested: None },
    Prop { name: "DesignTime", kind: 1, flags: 8, nested: None },
    Prop { name: "ColorWriteFlags", kind: 3, flags: 0, nested: None },
    Prop { name: "ClipChildren", kind: 1, flags: 0, nested: None },
    Prop { name: "EnableEffects", kind: 1, flags: 0, nested: None },
    Prop { name: "DisableFocusRecursion", kind: 1, flags: 0, nested: None },
];

static T_8244D498: [Prop; 13] = [
    Prop { name: "ClassOverride", kind: 5, flags: 0, nested: None },
    Prop { name: "Visual", kind: 5, flags: 0, nested: None },
    Prop { name: "Enabled", kind: 1, flags: 0, nested: None },
    Prop { name: "UnfocusedInput", kind: 1, flags: 0, nested: None },
    Prop { name: "NavLeft", kind: 5, flags: 0, nested: None },
    Prop { name: "NavRight", kind: 5, flags: 0, nested: None },
    Prop { name: "NavUp", kind: 5, flags: 0, nested: None },
    Prop { name: "NavDown", kind: 5, flags: 0, nested: None },
    Prop { name: "NavTabForward", kind: 5, flags: 0, nested: None },
    Prop { name: "NavTabBackward", kind: 5, flags: 0, nested: None },
    Prop { name: "Text", kind: 5, flags: 4, nested: None },
    Prop { name: "PointSize", kind: 4, flags: 6, nested: None },
    Prop { name: "ImagePath", kind: 5, flags: 16, nested: None },
];

static T_8245F1F0: [Prop; 1] = [
    Prop { name: "File", kind: 5, flags: 0, nested: None },
];

static T_82460840: [Prop; 3] = [
    Prop { name: "Cue", kind: 5, flags: 0, nested: None },
    Prop { name: "SoundBank", kind: 5, flags: 16, nested: None },
    Prop { name: "WaveBank", kind: 5, flags: 16, nested: None },
];

static T_824AD288: [Prop; 5] = [
    Prop { name: "File", kind: 5, flags: 0, nested: None },
    Prop { name: "SizeMode", kind: 3, flags: 0, nested: None },
    Prop { name: "Pause", kind: 1, flags: 0, nested: None },
    Prop { name: "Loop", kind: 1, flags: 8, nested: None },
    Prop { name: "Volume", kind: 4, flags: 0, nested: None },
];

pub static CLASSES: [Class; 49] = [
    Class { name: "XuiAvatar", parent: Some("XuiControl"), props: &T_8214E258 },
    Class { name: "LuaScene", parent: Some("XuiScene"), props: &[] },
    Class { name: "CommandConsoleScene", parent: Some("XuiScene"), props: &[] },
    Class { name: "XuiCanvas", parent: Some("XuiElement"), props: &[] },
    Class { name: "XuiText", parent: Some("XuiElement"), props: &T_82448718 },
    Class { name: "XuiLabel", parent: Some("XuiControl"), props: &[] },
    Class { name: "XuiVisual", parent: Some("XuiElement"), props: &[] },
    Class { name: "XuiButton", parent: Some("XuiControl"), props: &T_8243BB60 },
    Class { name: "XuiNavButton", parent: Some("XuiButton"), props: &T_82448DA0 },
    Class { name: "XuiBackButton", parent: Some("XuiButton"), props: &[] },
    Class { name: "XuiCheckbox", parent: Some("XuiControl"), props: &T_8243BCC8 },
    Class { name: "XuiRadioButton", parent: Some("XuiControl"), props: &T_8243BD80 },
    Class { name: "XuiRadioGroup", parent: Some("XuiControl"), props: &[] },
    Class { name: "XuiTextPresenter", parent: Some("XuiElement"), props: &T_82448F20 },
    Class { name: "XuiGroup", parent: Some("XuiElement"), props: &[] },
    Class { name: "XuiSound", parent: Some("XuiElement"), props: &T_82449220 },
    Class { name: "XuiScrollEnd", parent: Some("XuiControl"), props: &T_82445C50 },
    Class { name: "XuiScrollBar", parent: Some("XuiControl"), props: &T_82445DC8 },
    Class { name: "XuiList", parent: Some("XuiControl"), props: &T_82449778 },
    Class { name: "XuiComboBox", parent: Some("XuiControl"), props: &T_82440450 },
    Class { name: "XuiComboBoxList", parent: Some("XuiList"), props: &[] },
    Class { name: "XuiProgressBar", parent: Some("XuiControl"), props: &T_82449968 },
    Class { name: "XuiSlider", parent: Some("XuiControl"), props: &T_82449AC0 },
    Class { name: "XuiEdit", parent: Some("XuiControl"), props: &T_82449D80 },
    Class { name: "XuiCaret", parent: Some("XuiControl"), props: &[] },
    Class { name: "XuiMessageBox", parent: Some("XuiScene"), props: &[] },
    Class { name: "XuiImagePresenter", parent: Some("XuiElement"), props: &T_82440C88 },
    Class { name: "XuiImage", parent: Some("XuiElement"), props: &T_82449FF8 },
    Class { name: "XuiTabScene", parent: Some("XuiScene"), props: &T_8244A158 },
    Class { name: "XuiNineGrid", parent: Some("XuiElement"), props: &T_8244A3C0 },
    Class { name: "XuiTextureEffect", parent: Some("XuiEffect"), props: &T_82446550 },
    Class { name: "XuiGrayscaleEffect", parent: Some("XuiEffect"), props: &T_82441950 },
    Class { name: "XuiRecolorEffect", parent: Some("XuiEffect"), props: &T_824469C0 },
    Class { name: "XuiBrightPassEffect", parent: Some("XuiEffect"), props: &T_82446AE8 },
    Class { name: "XuiEdgeEffect", parent: Some("XuiEffect"), props: &[] },
    Class { name: "XuiHBlurEffect", parent: Some("XuiEffect"), props: &T_82441F18 },
    Class { name: "XuiVBlurEffect", parent: Some("XuiEffect"), props: &T_82442148 },
    Class { name: "XuiElement", parent: None, props: &T_8244CE40 },
    Class { name: "XuiControl", parent: Some("XuiElement"), props: &T_8244D498 },
    Class { name: "XuiTransition", parent: Some("XuiElement"), props: &[] },
    Class { name: "XuiListItem", parent: Some("XuiCheckbox"), props: &T_82449440 },
    Class { name: "XuiEffect", parent: Some("XuiElement"), props: &T_8244A678 },
    Class { name: "XuiHVBlurEffect", parent: Some("XuiEffect"), props: &T_82441CE0 },
    Class { name: "XuiFigure", parent: Some("XuiElement"), props: &T_82445660 },
    Class { name: "XuiScene", parent: Some("XuiControl"), props: &T_82448B18 },
    Class { name: "XuiCommonList", parent: Some("XuiList"), props: &T_82449810 },
    Class { name: "XuiSoundXAudio", parent: Some("XuiSound"), props: &T_8245F1F0 },
    Class { name: "XuiSoundXACT", parent: Some("XuiSound"), props: &T_82460840 },
    Class { name: "XuiVideo", parent: Some("XuiElement"), props: &T_824AD288 },
];
