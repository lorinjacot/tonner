pub struct KeyEvent {
    pub physical_key: PhysicalKey,
}

/// Code representing the physical location of a physical key.
/// This conforms to the UI Events Specification's [`KeyboardEvent.code`].
///
/// [`KeyboardEvent.code`]: https://w3c.github.io/uievents-code/#code-value-tables
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PhysicalKey {
    // Alphanumeric Section, Writing System Keys
    /// <kbd>`~</kbd> on a US keyboard. This is the <kbd>半角/全角/漢字</kbd> (hankaku/zenkaku/kanji) key on Japanese keyboards
    Backquote,
    /// Used for both the US <kbd>\\|</kbd> (on the 101-key layout) and also for the key located between the <kbd>"</kbd> and <kbd>Enter</kbd> keys on row C of the 102-, 104- and 106-key layouts. Labelled <kbd>#~</kbd> on a UK (102) keyboard.
    Backslash,
    /// <kbd>[{</kbd> on a US keyboard.
    BracketLeft,
    /// <kbd>]}</kbd> on a US keyboard.
    BracketRight,
    /// <kbd>,<</kbd> on a US keyboard.
    Comma,
    /// <kbd>0)</kbd> on a US keyboard.
    Digit0,
    /// <kbd>1!</kbd> on a US keyboard.
    Digit1,
    /// <kbd>2@</kbd> on a US keyboard.
    Digit2,
    /// <kbd>3#</kbd> on a US keyboard.
    Digit3,
    /// <kbd>4$</kbd> on a US keyboard.
    Digit4,
    /// <kbd>5%</kbd> on a US keyboard.
    Digit5,
    /// <kbd>6^</kbd> on a US keyboard.
    Digit6,
    /// <kbd>7&</kbd> on a US keyboard.
    Digit7,
    /// <kbd>8*</kbd> on a US keyboard.
    Digit8,
    /// <kbd>9(</kbd> on a US keyboard.
    Digit9,
    /// <kbd>=+</kbd> on a US keyboard.
    Equal,
    /// Located between the left <kbd>Shift</kbd> and <kbd>Z</kbd> keys. Labelled <kbd>\|</kbd> on a UK keyboard.
    IntlBackslash,
    /// Located between the <kbd>/</kbd> and right <kbd>Shift</kbd> keys. Labelled <kbd>\ろ</kbd> (ro) on a Japanese keyboard.
    IntlRo,
    /// Located between the <kbd>=</kbd> and <kbd>Backspace</kbd> keys. Labelled <kbd>¥</kbd> (yen) on a Japanese keyboard. <kbd>\\/</kbd> on a Russian keyboard.
    IntlYen,
    /// <kbd>a</kbd> on a US keyboard. Labelled <kbd>q</kbd> on an AZERTY (e.g., French) keyboard.
    KeyA,
    /// <kbd>b</kbd> on a US keyboard.
    KeyB,
    /// <kbd>c</kbd> on a US keyboard.
    KeyC,
    /// <kbd>d</kbd> on a US keyboard.
    KeyD,
    /// <kbd>e</kbd> on a US keyboard.
    KeyE,
    /// <kbd>f</kbd> on a US keyboard.
    KeyF,
    /// <kbd>g</kbd> on a US keyboard.
    KeyG,
    /// <kbd>h</kbd> on a US keyboard.
    KeyH,
    /// <kbd>i</kbd> on a US keyboard.
    KeyI,
    /// <kbd>j</kbd> on a US keyboard.
    KeyJ,
    /// <kbd>k</kbd> on a US keyboard.
    KeyK,
    /// <kbd>l</kbd> on a US keyboard.
    KeyL,
    /// <kbd>m</kbd> on a US keyboard.
    KeyM,
    /// <kbd>n</kbd> on a US keyboard.
    KeyN,
    /// <kbd>o</kbd> on a US keyboard.
    KeyO,
    /// <kbd>p</kbd> on a US keyboard.
    KeyP,
    /// <kbd>q</kbd> on a US keyboard. Labelled <kbd>a</kbd> on an AZERTY (e.g., French) keyboard.
    KeyQ,
    /// <kbd>r</kbd> on a US keyboard.
    KeyR,
    /// <kbd>s</kbd> on a US keyboard.
    KeyS,
    /// <kbd>t</kbd> on a US keyboard.
    KeyT,
    /// <kbd>u</kbd> on a US keyboard.
    KeyU,
    /// <kbd>v</kbd> on a US keyboard.
    KeyV,
    /// <kbd>w</kbd> on a US keyboard. Labelled <kbd>z</kbd> on an AZERTY (e.g., French) keyboard.
    KeyW,
    /// <kbd>x</kbd> on a US keyboard.
    KeyX,
    /// <kbd>y</kbd> on a US keyboard. Labelled <kbd>z</kbd> on a QWERTZ (e.g., German) keyboard.
    KeyY,
    /// <kbd>z</kbd> on a US keyboard. Labelled <kbd>w</kbd> on an AZERTY (e.g., French) keyboard, and <kbd>y</kbd> on a QWERTZ (e.g., German) keyboard.
    KeyZ,
    /// <kbd>-_</kbd> on a US keyboard.
    Minus,
    /// <kbd>.></kbd> on a US keyboard.
    Period,
    /// <kbd>'"</kbd> on a US keyboard.
    Quote,
    /// <kbd>;:</kbd> on a US keyboard.
    Semicolon,
    /// <kbd>/?</kbd> on a US keyboard.
    Slash,

    // Alphanumeric Section, Functional Keys
    /// <kbd>Alt</kbd>, <kbd>Option</kbd> or <kbd>⌥</kbd>.
    AltLeft,
    /// <kbd>Alt</kbd>, <kbd>Option</kbd> or <kbd>⌥</kbd>. This is labelled <kbd>AltGr</kbd> key on many keyboard layouts.
    AltRight,
    /// <kbd>Backspace</kbd> or <kbd>⌫</kbd>. Labelled <kbd>Delete</kbd> on Apple keyboards.
    Backspace,
    /// <kbd>CapsLock</kbd> or <kbd>⇪</kbd>
    CapsLock,
    /// The application context menu key, which is typically found between the right <kbd>Meta</kbd> key and the right <kbd>Control</kbd> key.
    ContextMenu,
    /// <kbd>Control</kbd> or <kbd>⌃</kbd>
    ControlLeft,
    /// <kbd>Control</kbd> or <kbd>⌃</kbd>
    ControlRight,
    /// <kbd>Enter</kbd> or <kbd>↵</kbd>. Labelled Return on <kbd>Apple</kbd> keyboards.
    Enter,
    /// The Windows, <kbd>⌘</kbd>, <kbd>Command</kbd> or other OS symbol key.
    MetaLeft,
    /// The Windows, <kbd>⌘</kbd>, <kbd>Command</kbd> or other OS symbol key.
    MetaRight,
    /// <kbd>Shift</kbd> or <kbd>⇧</kbd>
    ShiftLeft,
    /// <kbd>Shift</kbd> or <kbd>⇧</kbd>
    ShiftRight,
    /// <kbd> </kbd> (space)
    Space,
    /// <kbd>Tab</kbd> or <kbd>⇥</kbd>
    Tab,

    /// Japanese: <kbd>変換</kbd> (henkan)
    Convert,
    /// Japanese: <kbd>カタカナ/ひらがな/ローマ字</kbd> (katakana/hiragana/romaji)
    KanaMode,
    /// Korean: HangulMode <kbd>한/영</kbd> (han/yeong)
    ///
    /// Japanese (Mac keyboard): <kbd>かな</kbd> (kana)
    Lang1,
    /// Korean: Hanja <kbd>한자</kbd> (hanja)
    /// Japanese (Mac keyboard): <kbd>英数</kbd> (eisu)
    Lang2,
    /// Japanese (word-processing keyboard): Katakana
    Lang3,
    /// Japanese (word-processing keyboard): Hiragana
    Lang4,
    /// Japanese (word-processing keyboard): Zenkaku/Hankaku
    Lang5,
    /// Japanese: <kbd>無変換</kbd> (muhenkan)
    NonConvert,

    // Control Pad Section
    /// <kbd>⌦</kbd>. The forward delete key. Note that on Apple keyboards, the key labelled <kbd>Delete</kbd> on the main part of the keyboard should be encoded as `Backspace`.
    Delete,
    /// <kbd>End</kbd> or <kbd>↘</kbd>
    End,
    /// <kbd>Help</kbd>. Not present on standard PC keyboards.
    Help,
    /// <kbd>Home</kbd> or <kbd>↖</kbd>
    Home,
    /// <kbd>Insert</kbd> or <kbd>Ins</kbd>. Not present on Apple keyboards.
    Insert,
    /// <kbd>Page Down</kbd>, <kbd>PgDn</kbd> or<kbd> ⇟</kbd>
    PageDown,
    /// <kbd>Page Up</kbd>, <kbd>PgUp</kbd> or <kbd>⇞</kbd>
    PageUp,

    // Arrow Pad Section
    /// <kbd>↓</kbd>
    ArrowDown,
    /// <kbd>←</kbd>
    ArrowLeft,
    /// <kbd>→</kbd>
    ArrowRight,
    /// <kbd>↑</kbd>
    ArrowUp,

    // Numpad Section
    /// On the Mac, the `NumLock` code should be used for the numpad <kbd>Clear</kbd> key.
    NumLock,
    /// <kbd>0 Ins</kbd> on a keyboard
    ///
    /// <kbd>0</kbd> on a phone or remote control
    Numpad0,
    /// <kbd>1 End</kbd> on a keyboard
    ///
    /// <kbd>1</kbd> or <kbd>1 QZ</kbd> on a phone or remote control
    Numpad1,
    /// <kbd>2 ↓</kbd> on a keyboard  
    ///
    /// <kbd>2 ABC</kbd> on a phone or remote control
    Numpad2,
    /// <kbd>3 PgDn</kbd> on a keyboard   
    ///
    /// <kbd>3 DEF</kbd> on a phone or remote control
    Numpad3,
    /// <kbd>4 ←</kbd> on a keyboard  
    ///
    /// <kbd>4 GHI</kbd> on a phone or remote control
    Numpad4,
    /// <kbd>5</kbd> on a keyboard
    ///
    /// <kbd>5 JKL</kbd> on a phone or remote control
    Numpad5,
    /// <kbd>6 →</kbd> on a keyboard  
    ///
    /// <kbd>6 MNO</kbd> on a phone or remote control
    Numpad6,
    /// <kbd>7 Home</kbd> on a keyboard   
    ///
    /// <kbd>7 PQRS</kbd> or <kbd>7 PRS</kbd> on a phone or remote control
    Numpad7,
    /// <kbd>8 ↑</kbd> on a keyboard  
    ///
    /// <kbd>8 TUV</kbd> on a phone or remote control
    Numpad8,
    /// <kbd>9 PgUp</kbd> on a keyboard   
    ///
    /// <kbd>9 WXYZ</kbd> or <kbd>9 WXY</kbd> on a phone or remote control
    Numpad9,
    /// <kbd>+</kbd>
    NumpadAdd,
    /// Found on the Microsoft Natural Keyboard.
    NumpadBackspace,
    /// <kbd>C</kbd> or <kbd>AC</kbd> (All Clear). Also for use with numpads that have a <kbd>Clear</kbd> key that is separate from the NumLock key. On the Mac, the numpad <kbd>Clear</kbd> key should always be encoded as `NumLock`.
    NumpadClear,
    /// <kbd>C</kbd>E (Clear Entry)
    NumpadClearEntry,
    /// <kbd>,</kbd> (thousands separator). For locales where the thousands separator is a "." (e.g., Brazil), this key may generate a <kbd>.</kbd>.
    NumpadComma,
    /// <kbd>. Del</kbd>. For locales where the decimal separator is "," (e.g., Brazil), this key may generate a <kbd>,</kbd>.
    NumpadDecimal,
    /// <kbd>/</kbd>
    NumpadDivide,
    NumpadEnter,
    /// <kbd>=</kbd>
    NumpadEqual,
    /// <kbd>#</kbd> on a phone or remote control device. This key is typically found below the <kbd>9</kbd> key and to the right of the <kbd>0</kbd> key.
    NumpadHash,
    /// <kbd>M+</kbd> Add current entry to the value stored in memory.
    NumpadMemoryAdd,
    /// <kbd>MC</kbd> Clear the value stored in memory.
    NumpadMemoryClear,
    /// <kbd>MR</kbd> Replace the current entry with the value stored in memory.
    NumpadMemoryRecall,
    /// <kbd>MS</kbd> Replace the value stored in memory with the current entry.
    NumpadMemoryStore,
    /// <kbd>M-</kbd> Subtract current entry from the value stored in memory.
    NumpadMemorySubtract,
    /// <kbd>*</kbd> on a keyboard. For use with numpads that provide mathematical operations (<kbd>+</kbd>, <kbd>-</kbd>, <kbd>*</kbd> and <kbd>/</kbd>).
    ///
    /// Use `NumpadStar` for the <kbd>*</kbd> key on phones and remote controls.
    NumpadMultiply,
    /// <kbd>(</kbd> Found on the Microsoft Natural Keyboard.
    NumpadParenLeft,
    /// <kbd>)</kbd> Found on the Microsoft Natural Keyboard.
    NumpadParenRight,
    /// <kbd>*</kbd> on a phone or remote control device. This key is typically found below the <kbd>7</kbd> key and to the left of the <kbd>0</kbd> key.   
    ///
    /// Use `NumpadMultiply` for the <kbd>*</kbd> key on numeric keypads.
    NumpadStar,
    /// <kbd>-</kbd>
    NumpadSubtract,

    // Function Section
    /// <kbd>Esc</kbd> or <kbd>⎋</kbd>
    Escape,
    /// <kbd>F1</kbd>
    F1,
    /// <kbd>F2</kbd>
    F2,
    /// <kbd>F3</kbd>
    F3,
    /// <kbd>F4</kbd>
    F4,
    /// <kbd>F5</kbd>
    F5,
    /// <kbd>F6</kbd>
    F6,
    /// <kbd>F7</kbd>
    F7,
    /// <kbd>F8</kbd>
    F8,
    /// <kbd>F9</kbd>
    F9,
    /// <kbd>F10</kbd>
    F10,
    /// <kbd>F11</kbd>
    F11,
    /// <kbd>F12</kbd>
    F12,
    /// <kbd>Fn</kbd> This is typically a hardware key that does not generate a separate code. Most keyboards do not place this key in the function section, but it is included here to keep it with related keys.
    Fn,
    /// <kbd>FLock</kbd> or <kbd>FnLock</kbd>. Function Lock key. Found on the Microsoft Natural Keyboard.
    FnLock,
    /// <kbd>PrtScr</kbd> SysRq or <kbd>Print Screen</kbd>
    PrintScreen,
    /// <kbd>Scroll Lock</kbd>
    ScrollLock,
    /// <kbd>Pause Break</kbd>
    Pause,

    // Media Keys
    /// Some laptops place this key to the left of the <kbd>↑</kbd> key.
    BrowserBack,
    BrowserFavorites,
    /// Some laptops place this key to the right of the <kbd>↑</kbd> key.
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    /// <kbd>Eject</kbd> or <kbd>⏏</kbd>. This key is placed in the function section on some Apple keyboards.
    Eject,
    /// Sometimes labelled My Computer on the keyboard
    LaunchApp1,
    /// Sometimes labelled Calculator on the keyboard
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    /// This key is placed in the function section on some Apple keyboards, replacing the <kbd>Eject</kbd> key.
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,

    // Legacy, Non-Standard and Special Keys
    Hyper,
    Super,
    Turbo,

    Abort,
    Resume,
    Suspend,

    /// Found on Sun’s USB keyboard.
    Again,
    /// Found on Sun’s USB keyboard.
    Copy,
    /// Found on Sun’s USB keyboard.
    Cut,
    /// Found on Sun’s USB keyboard.
    Find,
    /// Found on Sun’s USB keyboard.
    Open,
    /// Found on Sun’s USB keyboard.
    Paste,
    /// Found on Sun’s USB keyboard.
    Props,
    /// Found on Sun’s USB keyboard.
    Select,
    /// Found on Sun’s USB keyboard.
    Undo,

    /// Use for dedicated <kbd>ひらがな</kbd> key found on some Japanese word processing keyboards.
    Hiragana,
    /// Use for dedicated <kbd>カタカナ</kbd> key found on some Japanese word processing keyboards.
    Katakana,

    /// This value code should be used when no other value given in this specification is appropriate.
    Unidentified,
}

/// Reprensent the meaning of a non-text key. This conforms to the UI Events Specification's [`KeyboardEvent.key`].
///
/// [`KeyboardEvent.key`]: https://w3c.github.io/uievents-key/#named-key-attribute-values
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NamedKey {
    // Special Keys
    /// This key value is used when an implementation is unable to identify another key value, due to either hardware, platform, or software constraints. 
    Unidentified,

    // Modifier Keys
    /// The <kbd>Alt</kbd> (Alternative) key. 
    /// 
    /// This key enables the alternate modifier function for interpreting concurrent or subsequent keyboard input. 
    /// 
    /// This key value is also used for the Apple Option key.
    Alt,
    /// The Alternate Graphics (<kbd>AltGr</kbd> or <kbd>AltGraph</kbd>) key. This key is used enable the ISO Level 3 shift modifier (the standard <kbd>Shift</kbd> key is the level 2 modifier). See [ISO9995-1].
    AltGraph,
    /// The <kbd>Caps Lock</kbd> (Capital) key. Toggle capital character lock function for interpreting subsequent keyboard input event.
    CapsLock,
    /// The <kbd>Control</kbd> or <kbd>Ctrl</kbd> key, to enable control modifier function for interpreting concurrent or subsequent keyboard input.
    Control,
    /// The Function switch <kbd>Fn</kbd> key.
    /// 
    /// Activating this key simultaneously with another key changes that key’s value to an alternate character or function. This key is often handled directly in the keyboard hardware and does not usually generate key events.
    Fn,
    /// The Function-Lock (<kbd>FnLock</kbd> or <kbd>F-Lock</kbd>) key.
    /// 
    /// Activating this key switches the mode of the keyboard to changes some keys' values to an alternate character or function. This key is often handled directly in the keyboard hardware and does not usually generate key events.
    FnLock,
    /// The <kbd>Meta</kbd> key, to enable meta modifier function for interpreting concurrent or subsequent keyboard input. This key value is used for the "Windows Logo" key and the Apple <kbd>Command</kbd> or <kbd>⌘</kbd> key.
    Meta,
    /// The <kbd>NumLock</kbd> or Number Lock key, to toggle numpad mode function for interpreting subsequent keyboard input.
    NumLock,
    /// The <kbd>Scroll Lock</kbd> key, to toggle between scrolling and cursor movement modes.
    ScrollLock,
    /// The <kbd>Shift</kbd> key, to enable shift modifier function for interpreting concurrent or subsequent keyboard input.
    Shift,
    /// The Symbol modifier key (used on some virtual keyboards).
    Symbol,
    /// The Symbol Lock key. 
    SymbolLock,

    // Legacy modifier keys
    /// The <kbd>Hyper</kbd> key.
    Hyper,
    /// The <kbd>Super</kbd> key. 
    Super,

    // Whitespace Keys
    /// The <kbd>Enter</kbd> or <kbd>↵</kbd> key, to activate current selection or accept current input.<br>This key value is also used for the <kbd>Return</kbd> (Macintosh numpad) key.<br>This key value is also used for the Android `KEYCODE_DPAD_CENTER`.
    Enter,
    /// The Horizontal Tabulation <kbd>Tab</kbd> key.
    Tab,

    // Navigation Keys
    /// The down arrow key, to navigate or traverse downward. (`KEYCODE_DPAD_DOWN`)
    ArrowDown,
    /// The left arrow key, to navigate or traverse leftward. (`KEYCODE_DPAD_LEFT`)
    ArrowLeft,
    /// The right arrow key, to navigate or traverse rightward. (`KEYCODE_DPAD_RIGHT`)
    ArrowRight,
    /// The up arrow key, to navigate or traverse upward. (`KEYCODE_DPAD_UP`)
    ArrowUp,
    /// The End key, used with keyboard entry to go to the end of content (`KEYCODE_MOVE_END`).
    End,
    /// The Home key, used with keyboard entry, to go to start of content (`KEYCODE_MOVE_HOME`).<br>For the mobile phone <kbd>Home</kbd> key (which goes to the phone’s main screen), use "GoHome".
    Home,
    /// The Page Down key, to scroll down or display next page of content.
    PageDown,
    /// The Page Up key, to scroll up or display previous page of content. 
    PageUp,

    // Editing Keys
    /// The Backspace key. This key value is also used for the key labeled <kbd>Delete</kbd> on MacOS keyboards.
    Backspace,
    /// Remove the currently selected input.
    Clear,
    /// Copy the current selection. (APPCOMMAND_COPY)
    Copy,
    /// The Cursor Select (Crsel) key.
    CrSel,
    /// Cut the current selection. (APPCOMMAND_CUT)
    Cut,
    /// The Delete (Del) Key. This key value is also used for the key labeled <kbd>Delete</kbd> on MacOS keyboards when modified by the <kbd>Fn</kbd> key.
    Delete,
    /// The Erase to End of Field key. This key deletes all characters from the current cursor position to the end of the current field.
    EraseEof,
    /// The Extend Selection (Exsel) key.
    ExSel,
    /// The Insert (Ins) key, to toggle between text modes for insertion or overtyping. (`KEYCODE_INSERT`)
    Insert,
    /// The Paste key. (`APPCOMMAND_PASTE`)
    Paste,
    /// Redo the last action. (`APPCOMMAND_REDO`)
    Redo,
    /// Undo the last action. (`APPCOMMAND_UNDO`) 
    Undo,

    // UI Keys
    /// The Accept (Commit, OK) key. Accept current option or input method sequence conversion.
    Accept,
    /// The Again key, to redo or repeat an action.
    Again,
    /// The Attention (Attn) key.
    Attn,
    /// The Cancel key.
    Cancel,
    /// Show the application’s context menu. This key is commonly found between the right <kbd>Meta</kbd> key and the right <kbd>Control</kbd> key.
    ContextMenu,
    /// The <kbd>Esc</kbd> key. This key was originally used to initiate an escape sequence, but is now more generally used to exit or "escape" the current context, such as closing a dialog or exiting full screen mode.
    Escape,
    /// The Execute key.
    Execute,
    /// Open the Find dialog. (`APPCOMMAND_FIND`)
    Find,
    /// Open a help dialog or toggle display of help information. (`APPCOMMAND_HELP`, `KEYCODE_HELP`)
    Help,
    /// Pause the current state or application (as appropriate).   
    /// 
    /// Do not use this value for the <kbd>Pause</kbd> button on media controllers. Use `MediaPause` instead.
    Pause,
    /// Play or resume the current state or application (as appropriate).  
    /// 
    /// Do not use this value for the <kbd>Play</kbd> button on media controllers. Use `MediaPlay` instead.
    Play,
    /// The properties (Props) key.
    Props,
    /// The Select key.
    Select,
    /// The ZoomIn key. (`KEYCODE_ZOOM_IN`)
    ZoomIn,
    /// The ZoomOut key. (`KEYCODE_ZOOM_OUT`) 
    ZoomOut,

    // Device Keys
    /// The Brightness Down key. Typically controls the display brightness. (`KEYCODE_BRIGHTNESS_DOWN`)
    BrightnessDown,
    /// The Brightness Up key. Typically controls the display brightness. (`KEYCODE_BRIGHTNESS_UP`)
    BrightnessUp,
    /// Toggle removable media to eject (open) and insert (close) state. (`KEYCODE_MEDIA_EJECT`)
    Eject,
    /// The LogOff key.
    LogOff,
    /// Toggle power state. (`KEYCODE_POWER`)
    /// 
    /// Note: Some devices might not expose this key to the operating environment.
    Power,
    /// The <kbd>PowerOff</kbd> key. Sometime called <kbd>PowerDown</kbd>.
    PowerOff,
    /// The <kbd>Print Screen</kbd> or <kbd>SnapShot</kbd> key, to initiate print-screen function.
    PrintScreen,
    /// The Hibernate key. This key saves the current state of the computer to disk so that it can be restored. The computer will then shutdown.
    Hibernate,
    /// The Standby key. This key turns off the display and places the computer into a low-power mode without completely shutting down. It is sometimes labelled <kbd>Suspend</kbd> or <kbd>Sleep</kbd> key. (`KEYCODE_SLEEP`)
    Standby,
    /// The WakeUp key. (`KEYCODE_WAKEUP`) 
    WakeUp,

    // IME and Composition Keys
    /// The All Candidates key, to initiate the multi-candidate mode.
    AllCandidates,
    /// The Alphanumeric key.
    Alphanumeric,
    /// The Code Input key, to initiate the Code Input mode to allow characters to be entered by their code points.
    CodeInput,
    /// The Compose key, also known as Multi_key on the X Window System. This key acts in a manner similar to a dead key, triggering a mode where subsequent key presses are combined to produce a different character.
    Compose,
    /// The Convert key, to convert the current input method sequence.
    Convert,
    /// A dead key combining key. It may be any combining key from any keyboard layout. For example, on a PC/AT French keyboard, using a French mapping and without any modifier activated, this is the key value "U+0302" COMBINING CIRCUMFLEX ACCENT. In another layout this might be a different unicode combining key.<br>
    /// For applications that need to differentiate between specific combining characters, the associated compositionupdate events' data attribute provides the specific key value.
    Dead,
    /// The Final Mode Final key used on some Asian keyboards, to enable the final mode for IMEs.
    FinalMode,
    /// Switch to the first character group. (ISO/IEC 9995)
    GroupFirst,
    /// Switch to the last character group. (ISO/IEC 9995)
    GroupLast,
    /// Switch to the next character group. (ISO/IEC 9995)
    GroupNext,
    /// Switch to the previous character group. (ISO/IEC 9995)
    GroupPrevious,
    /// The Mode Change key, to toggle between or cycle through input modes of IMEs.
    ModeChange,
    /// The Next Candidate function key.
    NextCandidate,
    /// The NonConvert ("Don't Convert") key, to accept current input method sequence without conversion in IMEs.
    NonConvert,
    /// The Previous Candidate function key.
    PreviousCandidate,
    /// The Process key.
    Process,
    /// The Single Candidate function key. 
    SingleCandidate,

    // IME and Composition Keys, Keys specific to Japanese keyboards
    /// The Eisu key. This key may close the IME, but its purpose is defined by the current IME. (`KEYCODE_EISU`)
    Eisu,
    /// The (Half-Width) Characters key.
    Hankaku,
    /// The Hiragana (Japanese Kana characters) key.
    Hiragana,
    /// The Hiragana/Katakana toggle key. (`KEYCODE_KATAKANA_HIRAGANA`)
    HiraganaKatakana,
    /// The Kana Mode (Kana Lock) key. This key is used to enter hiragana mode (typically from romaji mode).
    KanaMode,
    /// The Kanji (Japanese name for ideographic characters of Chinese origin) Mode key. This key is typically used to switch to a hiragana keyboard for the purpose of converting input into kanji. (`KEYCODE_KANA`)
    KanjiMode,
    /// The Katakana (Japanese Kana characters) key.
    Katakana,
    /// The Roman characters function key.
    Romaji,
    /// The Zenkaku (Full-Width) Characters key.
    Zenkaku,
    /// The Zenkaku/Hankaku (full-width/half-width) toggle key. (`KEYCODE_ZENKAKU_HANKAKU`) 
    ZenkakuHankaku,

    // General-Purpose Function Keys
    /// The F1 key, a general purpose function key, as index 1.
    F1,
    /// The F2 key, a general purpose function key, as index 2.
    F2,
    /// The F3 key, a general purpose function key, as index 3.
    F3,
    /// The F4 key, a general purpose function key, as index 4.
    F4,
    /// The F5 key, a general purpose function key, as index 5.
    F5,
    /// The F6 key, a general purpose function key, as index 6.
    F6,
    /// The F7 key, a general purpose function key, as index 7.
    F7,
    /// The F8 key, a general purpose function key, as index 8.
    F8,
    /// The F9 key, a general purpose function key, as index 9.
    F9,
    /// The F10 key, a general purpose function key, as index 10.
    F10,
    /// The F11 key, a general purpose function key, as index 11.
    F11,
    /// The F12 key, a general purpose function key, as index 12.
    F12,
    /// General purpose virtual function key, as index 1.
    Soft1,
    /// General purpose virtual function key, as index 2.
    Soft2,
    /// General purpose virtual function key, as index 3.
    Soft3,
    /// General purpose virtual function key, as index 4. 
    Soft4,

    // Multimedia Keys
    /// Select next (numerically or logically) lower channel. (`APPCOMMAND_MEDIA_CHANNEL_DOWN`, `KEYCODE_CHANNEL_DOWN`)
    ChannelDown,
    /// Select next (numerically or logically) higher channel. (`APPCOMMAND_MEDIA_CHANNEL_UP`, `KEYCODE_CHANNEL_UP`)
    ChannelUp,
    /// Close the current document or message (Note: This doesn’t close the application). (`APPCOMMAND_CLOSE`)
    Close,
    /// Open an editor to forward the current message. (`APPCOMMAND_FORWARD_MAIL`)
    MailForward,
    /// Open an editor to reply to the current message. (`APPCOMMAND_REPLY_TO_MAIL`)
    MailReply,
    /// Send the current message. (`APPCOMMAND_SEND_MAIL`)
    MailSend,
    /// Close the current media, for example to close a CD or DVD tray. (`KEYCODE_MEDIA_CLOSE`)
    MediaClose,
    /// Initiate or continue forward playback at faster than normal speed, or increase speed if already fast forwarding. (`APPCOMMAND_MEDIA_FAST_FORWARD`, `KEYCODE_MEDIA_FAST_FORWARD`)
    MediaFastForward,
    /// Pause the currently playing media. (`APPCOMMAND_MEDIA_PAUSE`, `KEYCODE_MEDIA_PAUSE`)   
    /// 
    /// Media controller devices should use this value rather than "Pause" for their pause keys.
    MediaPause,
    /// Initiate or continue media playback at normal speed, if not currently playing at normal speed. (`APPCOMMAND_MEDIA_PLAY`, `KEYCODE_MEDIA_PLAY`)
    MediaPlay,
    /// Toggle media between play and pause states. (`APPCOMMAND_MEDIA_PLAY_PAUSE`, `KEYCODE_MEDIA_PLAY_PAUSE`)
    MediaPlayPause,
    /// Initiate or resume recording of currently selected media. (`APPCOMMAND_MEDIA_RECORD`, `KEYCODE_MEDIA_RECORD`)
    MediaRecord,
    /// Initiate or continue reverse playback at faster than normal speed, or increase speed if already rewinding. (`APPCOMMAND_MEDIA_REWIND`, `KEYCODE_MEDIA_REWIND`)
    MediaRewind,
    /// Stop media playing, pausing, forwarding, rewinding, or recording, if not already stopped. (`APPCOMMAND_MEDIA_STOP`, `KEYCODE_MEDIA_STOP`)
    MediaStop,
    /// Seek to next media or program track. (`APPCOMMAND_MEDIA_NEXTTRACK`, `KEYCODE_MEDIA_NEXT`)
    MediaTrackNext,
    /// Seek to previous media or program track. (`APPCOMMAND_MEDIA_PREVIOUSTRACK`, `KEYCODE_MEDIA_PREVIOUS`)
    MediaTrackPrevious,
    /// Open a new document or message. (`APPCOMMAND_NEW`)
    New,
    /// Open an existing document or message. (`APPCOMMAND_OPEN`)
    Open,
    /// Print the current document or message. (`APPCOMMAND_PRINT`)
    Print,
    /// Save the current document or message. (`APPCOMMAND_SAVE`)
    Save,
    /// Spellcheck the current document or selection. (`APPCOMMAND_SPELL_CHECK`) 
    SpellCheck,

    // Multimedia Numpad Keys
    /// The <kbd>11</kbd> key found on media numpads that have buttons from <kbd>1</kbd> ... <kbd>12</kbd>.
    Key11,
    /// The <kbd>12</kbd> key found on media numpads that have buttons from <kbd>1</kbd> ... <kbd>12</kbd>. 
    Key12,

    // Audio Keys
    /// Adjust audio balance leftward. (`VK_AUDIO_BALANCE_LEFT`)
    AudioBalanceLeft,
    /// Adjust audio balance rightward. (`VK_AUDIO_BALANCE_RIGHT`)
    AudioBalanceRight,
    /// Decrease audio bass boost or cycle down through bass boost states. (`APPCOMMAND_BASS_DOWN`, `VK_BASS_BOOST_DOWN`)
    AudioBassBoostDown,
    /// Toggle bass boost on/off. (`APPCOMMAND_BASS_BOOST`)
    AudioBassBoostToggle,
    /// Increase audio bass boost or cycle up through bass boost states. (`APPCOMMAND_BASS_UP`, `VK_BASS_BOOST_UP`)
    AudioBassBoostUp,
    /// Adjust audio fader towards front. (`VK_FADER_FRONT`)
    AudioFaderFront,
    /// Adjust audio fader towards rear. (`VK_FADER_REAR`)
    AudioFaderRear,
    /// Advance surround audio mode to next available mode. (`VK_SURROUND_MODE_NEXT`)
    AudioSurroundModeNext,
    /// Decrease treble. (`APPCOMMAND_TREBLE_DOWN`)
    AudioTrebleDown,
    /// Increase treble. (`APPCOMMAND_TREBLE_UP`)
    AudioTrebleUp,
    /// Decrease audio volume. (`APPCOMMAND_VOLUME_DOWN`, `KEYCODE_VOLUME_DOWN`)
    AudioVolumeDown,
    /// Increase audio volume. (`APPCOMMAND_VOLUME_UP`, `KEYCODE_VOLUME_UP`)
    AudioVolumeUp,
    /// Toggle between muted state and prior volume level. (`APPCOMMAND_VOLUME_MUTE`, `KEYCODE_VOLUME_MUTE`)
    AudioVolumeMute,
    /// Toggle the microphone on/off. (`APPCOMMAND_MIC_ON_OFF_TOGGLE`)
    MicrophoneToggle,
    /// Decrease microphone volume. (`APPCOMMAND_MICROPHONE_VOLUME_DOWN`)
    MicrophoneVolumeDown,
    /// Increase microphone volume. (`APPCOMMAND_MICROPHONE_VOLUME_UP`)
    MicrophoneVolumeUp,
    /// Mute the microphone. (`APPCOMMAND_MICROPHONE_VOLUME_MUTE`, `KEYCODE_MUTE`) 
    MicrophoneVolumeMute,

    // Speech Keys
    /// Show correction list when a word is incorrectly identified. (`APPCOMMAND_CORRECTION_LIST`)
    SpeechCorrectionList,
    /// Toggle between dictation mode and command/control mode. (`APPCOMMAND_DICTATE_OR_COMMAND_CONTROL_TOGGLE`) 
    SpeechInputToggle,

    // Application Keys
    /// The first generic "LaunchApplication" key. This is commonly associated with launching "My Computer", and may have a computer symbol on the key. (`APPCOMMAND_LAUNCH_APP1`)
    LaunchApplication1,
    /// The second generic "LaunchApplication" key. This is commonly associated with launching "Calculator", and may have a calculator symbol on the key. (`APPCOMMAND_LAUNCH_APP2`, `KEYCODE_CALCULATOR`)
    LaunchApplication2,
    /// The "Calendar" key. (`KEYCODE_CALENDAR`)
    LaunchCalendar,
    /// The "Contacts" key. (`KEYCODE_CONTACTS`)
    LaunchContacts,
    /// The "Mail" key. (`APPCOMMAND_LAUNCH_MAIL`)
    LaunchMail,
    /// The "Media Player" key. (`APPCOMMAND_LAUNCH_MEDIA_SELECT`)
    LaunchMediaPlayer,
    /// The "Music Player" key.
    LaunchMusicPlayer,
    /// The "Phone" key.
    LaunchPhone,
    /// The "Screen Saver" key.
    LaunchScreenSaver,
    /// The "Spreadsheet" key.
    LaunchSpreadsheet,
    /// The "Web Browser" key.
    LaunchWebBrowser,
    /// The "WebCam" key.
    LaunchWebCam,
    /// The "Word Processor" key. 
    LaunchWordProcessor,

    // Browser Keys
    /// Navigate to previous content or page in current history. (`APPCOMMAND_BROWSER_BACKWARD`)
    BrowserBack,
    /// Open the list of browser favorites. (`APPCOMMAND_BROWSER_FAVORITES`)
    BrowserFavorites,
    /// Navigate to next content or page in current history. (`APPCOMMAND_BROWSER_FORWARD`)
    BrowserForward,
    /// Go to the user’s preferred home page. (`APPCOMMAND_BROWSER_HOME`)
    BrowserHome,
    /// Refresh the current page or content. (`APPCOMMAND_BROWSER_REFRESH`)
    BrowserRefresh,
    /// Call up the user’s preferred search page. (`APPCOMMAND_BROWSER_SEARCH`)
    BrowserSearch,
    /// Stop loading the current page or content. (`APPCOMMAND_BROWSER_STOP`) 
    BrowserStop,

    // Mobile Phone Keys
    /// The Application switch key, which provides a list of recent apps to switch between. (`KEYCODE_APP_SWITCH`)
    AppSwitch,
    /// The Call key. (`KEYCODE_CALL`)
    Call,
    /// The Camera key. (`KEYCODE_CAMERA`)
    Camera,
    /// The Camera focus key. (`KEYCODE_FOCUS`)
    CameraFocus,
    /// The End Call key. (`KEYCODE_ENDCALL`)
    EndCall,
    /// The Back key. (`KEYCODE_BACK`)
    GoBack,
    /// The Home key, which goes to the phone’s main screen. (`KEYCODE_HOME`)
    GoHome,
    /// The Headset Hook key. (`KEYCODE_HEADSETHOOK`)
    HeadsetHook,
    /// The Last Number Redial key.
    LastNumberRedial,
    /// The Notification key. (`KEYCODE_NOTIFICATION`)
    Notification,
    /// Toggle between manner mode state: silent, vibrate, ring, ... (`KEYCODE_MANNER_MODE`)
    MannerMode,
    /// The Voice Dial key. 
    VoiceDial,

    // TV Keys
    /// Switch to viewing TV. (`KEYCODE_TV`)
    TV,
    /// TV 3D Mode. (`KEYCODE_3D_MODE`)
    TV3DMode,
    /// Toggle between antenna and cable input. (`KEYCODE_TV_ANTENNA_CABLE`)
    TVAntennaCable,
    /// Audio description. (`KEYCODE_TV_AUDIO_DESCRIPTION`)
    TVAudioDescription,
    /// Audio description mixing volume down. (`KEYCODE_TV_AUDIO_DESCRIPTION_MIX_DOWN`)
    TVAudioDescriptionMixDown,
    /// Audio description mixing volume up. (`KEYCODE_TV_AUDIO_DESCRIPTION_MIX_UP`)
    TVAudioDescriptionMixUp,
    /// Contents menu. (`KEYCODE_TV_CONTENTS_MENU`)
    TVContentsMenu,
    /// Contents menu. (`KEYCODE_TV_DATA_SERVICE`)
    TVDataService,
    /// Switch the input mode on an external TV. (`KEYCODE_TV_INPUT`)
    TVInput,
    /// Switch to component input #1. (`KEYCODE_TV_INPUT_COMPONENT_1`)
    TVInputComponent1,
    /// Switch to component input #2. (`KEYCODE_TV_INPUT_COMPONENT_2`)
    TVInputComponent2,
    /// Switch to composite input #1. (`KEYCODE_TV_INPUT_COMPOSITE_1`)
    TVInputComposite1,
    /// Switch to composite input #2. (`KEYCODE_TV_INPUT_COMPOSITE_2`)
    TVInputComposite2,
    /// Switch to HDMI input #1. (`KEYCODE_TV_INPUT_HDMI_1`)
    TVInputHDMI1,
    /// Switch to HDMI input #2. (`KEYCODE_TV_INPUT_HDMI_2`)
    TVInputHDMI2,
    /// Switch to HDMI input #3. (`KEYCODE_TV_INPUT_HDMI_3`)
    TVInputHDMI3,
    /// Switch to HDMI input #4. (`KEYCODE_TV_INPUT_HDMI_4`)
    TVInputHDMI4,
    /// Switch to VGA input #1. (`KEYCODE_TV_INPUT_VGA_1`)
    TVInputVGA1,
    /// Media context menu. (`KEYCODE_TV_MEDIA_CONTEXT_MENU`)
    TVMediaContext,
    /// Toggle network. (`KEYCODE_TV_NETWORK`)
    TVNetwork,
    /// Number entry. (`KEYCODE_TV_NUMBER_ENTRY`)
    TVNumberEntry,
    /// Toggle the power on an external TV. (`KEYCODE_TV_POWER`)
    TVPower,
    /// Radio. (`KEYCODE_TV_RADIO_SERVICE`)
    TVRadioService,
    /// Satellite. (`KEYCODE_TV_SATELLITE`)
    TVSatellite,
    /// Broadcast Satellite. (`KEYCODE_TV_SATELLITE_BS`)
    TVSatelliteBS,
    /// Communication Satellite. (`KEYCODE_TV_SATELLITE_CS`)
    TVSatelliteCS,
    /// Toggle between available satellites. (`KEYCODE_TV_SATELLITE_SERVICE`)
    TVSatelliteToggle,
    /// Analog Terrestrial. (`KEYCODE_TV_TERRESTRIAL_ANALOG`)
    TVTerrestrialAnalog,
    /// Digital Terrestrial. (`KEYCODE_TV_TERRESTRIAL_DIGITAL`)
    TVTerrestrialDigital,
    /// Timer programming. (KEYCODE_TV_TIMER_PROGRAMMING`)` 
    TVTimer,

    // Media Controller Keys
    /// Switch the input mode on an external AVR (audio/video receiver). (`KEYCODE_AVR_INPUT`)
    AVRInput,
    /// Toggle the power on an external AVR (audio/video receiver). (`KEYCODE_AVR_POWER`)
    AVRPower,
    /// General purpose color-coded media function key, as index 0 (red). (`VK_COLORED_KEY_0`, `KEYCODE_PROG_RED`)
    ColorF0Red,
    /// General purpose color-coded media function key, as index 1 (green). (`VK_COLORED_KEY_1`, `KEYCODE_PROG_GREEN`)
    ColorF1Green,
    /// General purpose color-coded media function key, as index 2 (yellow). (`VK_COLORED_KEY_2`, `KEYCODE_PROG_YELLOW`)
    ColorF2Yellow,
    /// General purpose color-coded media function key, as index 3 (blue). (`VK_COLORED_KEY_3`, `KEYCODE_PROG_BLUE`)
    ColorF3Blue,
    /// General purpose color-coded media function key, as index 4 (grey). (`VK_COLORED_KEY_4`)
    ColorF4Grey,
    /// General purpose color-coded media function key, as index 5 (brown). (`VK_COLORED_KEY_5`)
    ColorF5Brown,
    /// Toggle the display of Closed Captions. (`VK_CC`, `KEYCODE_CAPTIONS`)
    ClosedCaptionToggle,
    /// Adjust brightness of device, by toggling between or cycling through states. (`VK_DIMMER`)
    Dimmer,
    /// Swap video sources. (`VK_DISPLAY_SWAP`)
    DisplaySwap,
    /// Select Digital Video Rrecorder. (`KEYCODE_DVR`)
    DVR,
    /// Exit the current application. (`VK_EXIT`)
    Exit,
    /// Clear program or content stored as favorite 0. (`VK_CLEAR_FAVORITE_0`)
    FavoriteClear0,
    /// Clear program or content stored as favorite 1. (`VK_CLEAR_FAVORITE_1`)
    FavoriteClear1,
    /// Clear program or content stored as favorite 2. (`VK_CLEAR_FAVORITE_2`)
    FavoriteClear2,
    /// Clear program or content stored as favorite 3. (`VK_CLEAR_FAVORITE_3`)
    FavoriteClear3,
    /// Select (recall) program or content stored as favorite 0. (`VK_RECALL_FAVORITE_0`)
    FavoriteRecall0,
    /// Select (recall) program or content stored as favorite 1. (`VK_RECALL_FAVORITE_1`)
    FavoriteRecall1,
    /// Select (recall) program or content stored as favorite 2. (`VK_RECALL_FAVORITE_2`)
    FavoriteRecall2,
    /// Select (recall) program or content stored as favorite 3. (`VK_RECALL_FAVORITE_3`)
    FavoriteRecall3,
    /// Store current program or content as favorite 0. (`VK_STORE_FAVORITE_0`)
    FavoriteStore0,
    /// Store current program or content as favorite 1. (`VK_STORE_FAVORITE_1`)
    FavoriteStore1,
    /// Store current program or content as favorite 2. (`VK_STORE_FAVORITE_2`)
    FavoriteStore2,
    /// Store current program or content as favorite 3. (`VK_STORE_FAVORITE_3`)
    FavoriteStore3,
    /// Toggle display of program or content guide. (VK_GUIDE, `KEYCODE_GUIDE`)
    Guide,
    /// If guide is active and displayed, then display next day’s content. (`VK_NEXT_DAY`)
    GuideNextDay,
    /// If guide is active and displayed, then display previous day’s content. (`VK_PREV_DAY`)
    GuidePreviousDay,
    /// Toggle display of information about currently selected context or media. (`VK_INFO`, `KEYCODE_INFO`)
    Info,
    /// Toggle instant replay. (`VK_INSTANT_REPLAY`)
    InstantReplay,
    /// Launch linked content, if available and appropriate. (`VK_LINK`)
    Link,
    /// List the current program. (`VK_LIST`)
    ListProgram,
    /// Toggle display listing of currently available live content or programs. (`VK_LIVE`)
    LiveContent,
    /// Lock or unlock current content or program. (`VK_LOCK`)
    Lock,
    /// Show a list of media applications: audio/video players and image viewers. (`VK_APPS`)
    /// 
    /// Do not confuse this key value with the Windows' `VK_APPS` / `VK_CONTEXT_MENU` key, which is encoded as "ContextMenu".
    MediaApps,
    /// Audio track key. (`KEYCODE_MEDIA_AUDIO_TRACK`)
    MediaAudioTrack,
    /// Select previously selected channel or media. (VK_LAST, `KEYCODE_LAST_CHANNEL`)
    MediaLast,
    /// Skip backward to next content or program. (`KEYCODE_MEDIA_SKIP_BACKWARD`)
    MediaSkipBackward,
    /// Skip forward to next content or program. (`VK_SKIP`, `KEYCODE_MEDIA_SKIP_FORWARD`)
    MediaSkipForward,
    /// Step backward to next content or program. (`KEYCODE_MEDIA_STEP_BACKWARD`)
    MediaStepBackward,
    /// Step forward to next content or program. (`KEYCODE_MEDIA_STEP_FORWARD`)
    MediaStepForward,
    /// Media top menu. (`KEYCODE_MEDIA_TOP_MENU`)
    MediaTopMenu,
    /// Navigate in. (`KEYCODE_NAVIGATE_IN`)
    NavigateIn,
    /// Navigate to next key. (`KEYCODE_NAVIGATE_NEXT`)
    NavigateNext,
    /// Navigate out. (`KEYCODE_NAVIGATE_OUT`)
    NavigateOut,
    /// Navigate to previous key. (`KEYCODE_NAVIGATE_PREVIOUS`)
    NavigatePrevious,
    /// Cycle to next favorite channel (in favorites list). (`VK_NEXT_FAVORITE_CHANNEL`)
    NextFavoriteChannel,
    /// Cycle to next user profile (if there are multiple user profiles). (`VK_USER`)
    NextUserProfile,
    /// Access on-demand content or programs. (`VK_ON_DEMAND`)
    OnDemand,
    /// Pairing key to pair devices. (`KEYCODE_PAIRING`)
    Pairing,
    /// Move picture-in-picture window down. (`VK_PINP_DOWN`)
    PinPDown,
    /// Move picture-in-picture window. (`VK_PINP_MOVE`)
    PinPMove,
    /// Toggle display of picture-in-picture window. (`VK_PINP_TOGGLE`)
    PinPToggle,
    /// Move picture-in-picture window up. (`VK_PINP_UP`)
    PinPUp,
    /// Decrease media playback speed. (`VK_PLAY_SPEED_DOWN`)
    PlaySpeedDown,
    /// Reset playback to normal speed. (`VK_PLAY_SPEED_RESET`)
    PlaySpeedReset,
    /// Increase media playback speed. (`VK_PLAY_SPEED_UP`)
    PlaySpeedUp,
    /// Toggle random media or content shuffle mode. (`VK_RANDOM_TOGGLE`)
    RandomToggle,
    /// Not a physical key, but this key code is sent when the remote control battery is low. (`VK_RC_LOW_BATTERY`)
    RcLowBattery,
    /// Toggle or cycle between media recording speeds. (`VK_RECORD_SPEED_NEXT`)
    RecordSpeedNext,
    /// Toggle RF (radio frequency) input bypass mode (pass RF input directly to the RF output). (`VK_RF_BYPASS`)
    RfBypass,
    /// Toggle scan channels mode. (`VK_SCAN_CHANNELS_TOGGLE`)
    ScanChannelsToggle,
    /// Advance display screen mode to next available mode. (`VK_SCREEN_MODE_NEXT`)
    ScreenModeNext,
    /// Toggle display of device settings screen. (`VK_SETTINGS`, `KEYCODE_SETTINGS`)
    Settings,
    /// Toggle split screen mode. (`VK_SPLIT_SCREEN_TOGGLE`)
    SplitScreenToggle,
    /// Switch the input mode on an external STB (set top box). (`KEYCODE_STB_INPUT`)
    STBInput,
    /// Toggle the power on an external STB (set top box). (`KEYCODE_STB_POWER`)
    STBPower,
    /// Toggle display of subtitles, if available. (`VK_SUBTITLE`)
    Subtitle,
    /// Toggle display of teletext, if available (`VK_TELETEXT`, KEYCODE_TV_TELETEXT`)`.
    Teletext,
    /// Advance video mode to next available mode. (`VK_VIDEO_MODE_NEXT`)
    VideoModeNext,
    /// Cause device to identify itself in some manner, e.g., audibly or visibly. (`VK_WINK`)
    Wink,
    /// Toggle between full-screen and scaled content, or alter magnification level. (`VK_ZOOM`, `KEYCODE_TV_ZOOM_MODE`)
    ZoomToggle,
}
