use rmusix::models::Song;

fn song(id: &str) -> Song {
    Song {
        video_id: id.into(),
        title: format!("title {id}"),
        channel: "chan".into(),
        duration: None,
        thumbnail_url: None,
    }
}

#[test]
fn enqueue_advances_and_skips() {
    let mut q = rmusix::queue::QueueManager::new();
    q.push(song("a"));
    q.push(song("b"));
    q.push(song("c"));
    assert_eq!(q.len(), 3);

    // Play first, skip forward through the queue.
    assert_eq!(q.play_at(0).unwrap().video_id, "a");
    assert_eq!(q.next().unwrap().video_id, "b");
    assert_eq!(q.next().unwrap().video_id, "c");
    // Past the end stays None.
    assert!(q.next().is_none());

    // Back one.
    assert_eq!(q.previous().unwrap().video_id, "b");
}

#[test]
fn remove_and_reorder_keep_cursor() {
    let mut q = rmusix::queue::QueueManager::new();
    q.push(song("a"));
    q.push(song("b"));
    q.push(song("c"));
    q.play_at(1);
    assert_eq!(q.current().unwrap().video_id, "b");

    // Remove the current song: cursor clears.
    let removed = q.remove(1).unwrap();
    assert_eq!(removed.video_id, "b");
    assert!(q.current().is_none());

    // play_at(1) is now "c"; move it up.
    q.play_at(1);
    assert!(q.move_up(1));
    assert_eq!(q.songs()[0].video_id, "c");
    assert_eq!(q.index(), Some(0));
    assert!(q.move_down(0));
    assert_eq!(q.songs()[1].video_id, "c");

    q.clear();
    assert!(q.is_empty());
    assert!(q.current().is_none());
}
