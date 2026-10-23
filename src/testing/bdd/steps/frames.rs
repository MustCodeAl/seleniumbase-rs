//! Steps about iframes.

sb_steps! {
    /// Switches to an iframe.
    fn switch_to_frame(sb, frame: String) { sb.switch_to_frame(&frame).await }
        "Switch to iframe {frame:string}", "Switch to frame {frame:string}",
        "User switches to iframe {frame:string}", "User switches to frame {frame:string}";

    /// Switches back to the page.
    fn switch_to_default_content(sb) { sb.switch_to_default_content().await }
        "Switch to default content", "Exit from iframes", "Exit from frames",
        "User switches to default content", "User exits from iframes", "User exits from frames";

    /// Switches to the parent of the current frame.
    fn switch_to_parent_frame(sb) { sb.switch_to_parent_frame().await }
        "Switch to parent frame", "Exit current iframe", "Exit current frame",
        "User switches to parent frame", "User exits current iframe", "User exits current frame";

    /// Makes an iframe the content that the next steps use.
    fn set_content_to_frame(sb, frame: String) { sb.set_content_to_frame(&frame).await }
        "Set content to frame {frame:string}", "User sets content to frame {frame:string}";

    /// Makes the page the content that the next steps use.
    fn set_content_to_default(sb) { sb.set_content_to_default().await }
        "Set content to default", "User sets content to default";

    /// Makes the parent frame the content that the next steps use.
    fn set_content_to_parent(sb) { sb.set_content_to_parent().await }
        "Set content to parent", "User sets content to parent";
}
