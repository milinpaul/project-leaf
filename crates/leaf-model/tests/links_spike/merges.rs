//! Two peers editing the same block text around a page link, then syncing.

use loro::LoroResult;

use crate::{ATOM, fork, insert_link, marked_runs, peer, render, sync};

#[test]
fn deleted_on_one_peer_while_the_other_formats_it_stays_deleted() -> LoroResult<()> {
    let a = peer(1, true)?;
    let text = a.get_text("text");
    text.insert(0, "go  now")?;
    insert_link(&a, &text, 3, "trip")?;
    let b = fork(&a, 2, true)?;

    text.delete(3, 1)?;
    a.commit();
    let other = b.get_text("text");
    other.mark(0..other.len_unicode(), "bold", true)?;
    b.commit();

    sync(&a, &b)?;
    assert_eq!(render(&text), "go  now");
    assert_eq!(render(&text), render(&other));
    assert_eq!(marked_runs(&text, "bold"), vec!["go  now"]);
    Ok(())
}

#[test]
fn deleted_on_one_peer_while_the_other_types_beside_it_keeps_the_typing() -> LoroResult<()> {
    let a = peer(1, true)?;
    let text = a.get_text("text");
    text.insert(0, "a b")?;
    insert_link(&a, &text, 2, "trip")?;
    let b = fork(&a, 2, true)?;

    text.delete(2, 1)?;
    a.commit();
    let other = b.get_text("text");
    other.insert(3, " (booked)")?;
    b.commit();

    sync(&a, &b)?;
    assert_eq!(render(&text), "a  (booked)b");
    assert_eq!(render(&text), render(&other));
    Ok(())
}

#[test]
fn a_link_added_inside_text_the_other_peer_deleted_survives() -> LoroResult<()> {
    let a = peer(1, true)?;
    let text = a.get_text("text");
    text.insert(0, "old words")?;
    a.commit();
    let b = fork(&a, 2, true)?;

    text.delete(0, 9)?;
    a.commit();
    let other = b.get_text("text");
    insert_link(&b, &other, 4, "trip")?;

    sync(&a, &b)?;
    // Inserts are never lost: the link outlives the words around it.
    assert_eq!(render(&text), "[[trip]]");
    assert_eq!(render(&text), render(&other));
    Ok(())
}

#[test]
fn the_same_link_added_on_both_peers_is_kept_twice() -> LoroResult<()> {
    let a = peer(1, true)?;
    let text = a.get_text("text");
    text.insert(0, "see ")?;
    a.commit();
    let b = fork(&a, 2, true)?;

    insert_link(&a, &text, 4, "trip")?;
    let other = b.get_text("text");
    insert_link(&b, &other, 4, "trip")?;

    sync(&a, &b)?;
    // Two links side by side, like two people typing the same word: visible, and one Backspace away.
    assert_eq!(render(&text), "see [[trip]][[trip]]");
    assert_eq!(render(&text), render(&other));
    Ok(())
}

#[test]
fn retargeting_while_the_other_peer_formats_keeps_both_changes() -> LoroResult<()> {
    let a = peer(1, true)?;
    let text = a.get_text("text");
    text.insert(0, "x")?;
    insert_link(&a, &text, 1, "trip")?;
    let b = fork(&a, 2, true)?;

    // Peer 1 points the link at another page (re-marking the same character); peer 2 makes it bold.
    text.mark(1..2, "page", "itinerary")?;
    a.commit();
    let other = b.get_text("text");
    other.mark(0..2, "bold", true)?;
    b.commit();

    sync(&a, &b)?;
    assert_eq!(render(&text), "x[[itinerary]]");
    assert_eq!(render(&text), render(&other));
    assert_eq!(marked_runs(&text, "bold").concat(), format!("x{ATOM}"));
    Ok(())
}

#[test]
fn retargeted_on_both_peers_settles_on_one_page() -> LoroResult<()> {
    let a = peer(1, true)?;
    let text = a.get_text("text");
    insert_link(&a, &text, 0, "trip")?;
    let b = fork(&a, 2, true)?;

    text.mark(0..1, "page", "bookings")?;
    a.commit();
    let other = b.get_text("text");
    other.mark(0..1, "page", "budget")?;
    b.commit();

    sync(&a, &b)?;
    // Last writer wins (the higher peer on a tie): one target, the same on both peers, never a mix.
    assert_eq!(render(&text), render(&other));
    assert!(matches!(render(&text).as_str(), "[[bookings]]" | "[[budget]]"));
    Ok(())
}
