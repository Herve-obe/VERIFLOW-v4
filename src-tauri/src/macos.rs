//! Correctifs propres à macOS.
//!
//! Sous macOS 10.15 (Catalina), après un agrandissement de la fenêtre, la vue
//! web peut garder son ancienne hauteur : bande vide en haut de la fenêtre et
//! clics décalés par rapport à l'affichage. À chaque redimensionnement, la
//! vue web est recalée sur la taille de son conteneur (taille modifiée puis
//! rétablie, pour que WebKit recalcule sa zone d'affichage).

use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_core_foundation::CGRect;

pub fn refresh_webview_frame(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|platform| {
        let webview = platform.inner() as *mut AnyObject;
        if webview.is_null() {
            return;
        }
        // SAFETY : `inner()` est le WKWebView vivant de cette fenêtre ; le rappel
        // s'exécute sur le fil principal, seul autorisé à toucher aux vues AppKit.
        unsafe {
            let parent: *mut AnyObject = msg_send![webview, superview];
            if parent.is_null() {
                return;
            }
            let bounds: CGRect = msg_send![parent, bounds];
            let mut nudged = bounds;
            nudged.size.height = (bounds.size.height - 1.0).max(0.0);
            let _: () = msg_send![webview, setFrame: nudged];
            let _: () = msg_send![webview, setFrame: bounds];
        }
    });
}
