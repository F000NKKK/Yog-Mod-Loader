use super::*;

#[test]
fn packages_are_namespaces_under_game() {
    let (ns, name) = of_class("net/minecraft/world/entity/player/Player").unwrap();
    assert_eq!(
        (ns.join("."), name.as_str()),
        ("World.Entity.Player".to_string(), "Player")
    );
    let (ns, name) = of_class("net/minecraft/network/chat/OutgoingChatMessage$Player").unwrap();
    assert_eq!(
        (ns.join("."), name.as_str()),
        ("Network.Chat".to_string(), "OutgoingChatMessage.Player")
    );
    let (ns, _) = of_class("com/mojang/blaze3d/vertex/PoseStack").unwrap();
    assert_eq!(ns.join("."), "Mojang.Blaze3d.Vertex");
    assert!(of_class("java/lang/Object").is_none());
}

#[test]
fn namespaces_are_packages_back() {
    let ns = |s: &str| s.split('.').map(String::from).collect::<Vec<_>>();
    assert_eq!(
        class_of(&ns("World.Entity.Player"), "Player"),
        "net/minecraft/world/entity/player/Player"
    );
    assert_eq!(
        class_of(&ns("Network.Chat"), "OutgoingChatMessage.Player"),
        "net/minecraft/network/chat/OutgoingChatMessage$Player"
    );
    assert_eq!(
        class_of(&ns("Mojang.Blaze3d.Vertex"), "PoseStack"),
        "com/mojang/blaze3d/vertex/PoseStack"
    );
    assert_eq!(class_of(&[], "Minecraft"), "net/minecraft/Minecraft");
}
