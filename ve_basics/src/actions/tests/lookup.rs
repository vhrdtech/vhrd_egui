use super::*;

#[test]
fn find_matches_titles_and_ids_best_first_and_bounded() {
    let mut actions = app();
    actions.set_enabled("file.save", Err("No document open".into()));

    let found = actions.find("save", 10);
    let ids: Vec<_> = found.iter().map(|f| f.id).collect();
    assert_eq!(
        ids,
        ["file.save", "file.save_as"],
        "disabled actions are found too"
    );
    assert_eq!(found[0].title_hits, [0, 1, 2, 3]);

    let found = actions.find("save as", 10);
    assert_eq!(found[0].id, "file.save_as");
    assert_eq!(found[0].title_hits, [0, 1, 2, 3, 5, 6]);

    // Word starts beat letters scattered in the middle.
    assert_eq!(actions.find("nr", 10)[0].id, "focus.next_region");

    // The id is searched too, a little below the title.
    assert_eq!(actions.find("theme", 10)[0].id, "view.theme");
    let found = actions.find("view.th", 10);
    assert_eq!(found[0].id, "view.theme");
    assert_eq!(found[0].title_hits, []);

    assert_eq!(actions.find("zzz", 10), []);
    assert_eq!(actions.find("e", 2).len(), 2);
    assert_eq!(actions.find(&"s".repeat(10_000), 10), []);
}

// Review 14: one fold on both sides, so every title finds itself.
#[test]
fn find_folds_case_the_same_on_both_sides() {
    let actions = Actions::from_list([
        Action::new("x.istanbul", "İstanbul öffnen"),
        Action::new("x.greek", "ΟΔΟΣ"),
        Action::new("x.size", "Größe ändern"),
    ]);
    for (query, id) in [
        ("İ", "x.istanbul"),
        ("istanbul", "x.istanbul"),
        ("ÖFF", "x.istanbul"),
        ("ΟΔΟΣ", "x.greek"),
        ("οδος", "x.greek"),
        ("οδοσ", "x.greek"),
        ("GRÖ", "x.size"),
    ] {
        let found = actions.find(query, 10);
        assert_eq!(found.first().map(|f| f.id), Some(id), "{query}");
    }
    assert_eq!(actions.find("İst", 1)[0].title_hits, [0, 1, 2]);
}

// Review 11: with no query the palette lists by menu order, like the menus.
#[test]
fn an_empty_query_lists_by_menu_order() {
    let actions = app();
    let ids: Vec<_> = actions.find("", 100).into_iter().map(|f| f.id).collect();
    assert_eq!(
        ids,
        [
            "file.recent.clear",
            "file.open",
            "file.save",
            "file.save_as",
            "file.delete",
            "edit.copy",
            "edit.find",
            "edit.paste",
            "edit.undo",
            "edit.redo",
            "view.play",
            "view.grid",
            "view.split",
            "view.theme",
            "view.zoom_in",
            "help.about",
            "focus.next_region",
            "tracks.remove",
        ]
    );
}

#[test]
fn the_menu_tree_follows_groups_and_order() {
    let actions = app();
    let menus = actions.menu_tree();
    let names: Vec<_> = menus.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        ["File", "Edit", "View", "Help"],
        "groupless actions are in none"
    );
    assert_eq!(
        menus[0].items,
        [
            MenuItem::Menu(MenuNode {
                name: "Recent".into(),
                items: vec![MenuItem::Action("file.recent.clear")]
            }),
            MenuItem::Action("file.open"),
            MenuItem::Action("file.save"),
            MenuItem::Action("file.save_as"),
            MenuItem::Action("file.delete"),
        ]
    );
    let menus = app().menu_order(&["View"]).menu_tree();
    let names: Vec<_> = menus.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["View", "Edit", "File", "Help"]);
}
