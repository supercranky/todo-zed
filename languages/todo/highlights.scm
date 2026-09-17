(heading) @title

(completed_task) @comment
(completed_task (completion_marker) @comment)
(completed_task (date) @comment)
(completed_task (body (body_item) @comment))
(completed_task (body (body_item (text) @comment)))
(completed_task (body (body_item (project) @comment)))
(completed_task (body (body_item (context) @comment)))
(completed_task (body (body_item (metadata) @comment)))

(unchecked_task) @text.muted
(unchecked_task (unchecked_marker) @text.muted)
(unchecked_task (body (body_item) @text.muted))
(unchecked_task (body (body_item (text) @text.muted)))
(unchecked_task (body (body_item (project) @text.muted)))
(unchecked_task (body (body_item (context) @text.muted)))
(unchecked_task (body (body_item (metadata) @text.muted)))

(pending_task) @text.muted
(pending_task (priority) @text.muted)
(pending_task (date) @text.muted)
(pending_task (body (body_item) @text.muted))
(pending_task (body (body_item (text) @text.muted)))
(pending_task (body (body_item (project) @text.muted)))
(pending_task (body (body_item (context) @text.muted)))
(pending_task (body (body_item (metadata) @text.muted)))

(plain_task) @text.muted
(plain_task (date) @text.muted)
(plain_task (body (body_item) @text.muted))
(plain_task (body (body_item (text) @text.muted)))
(plain_task (body (body_item (project) @text.muted)))
(plain_task (body (body_item (context) @text.muted)))
(plain_task (body (body_item (metadata) @text.muted)))

(priority) @keyword
