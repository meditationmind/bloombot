INSERT INTO bookmarks (record_id, user_id, guild_id, message_link, user_desc, occurred_at)
VALUES
    ('01JBPTWBXJNAKK288S3D89JK7G', '123', '123', 'https://foo.bar/1234', 'A bar of foo', CAST('2024-01-01 00:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPTWBXJNAKK288S3D89JK7H', '123', '123', 'https://foo.bar/1235', 'A baz of bat', CAST('2024-01-01 01:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPTWBXJNAKK288S3D89JK7I', '123', '123', 'https://foo.bar/1236', null, CAST('2024-01-01 02:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPTWBXJNAKK288S3D89JK7J', '123', '123', 'https://foo.bar/1237', 'A quu of quux', CAST('2024-01-01 03:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPTWBXJNAKK288S3D89JK7K', '124', '123', 'https://foo.bar/1238', 'A quick brown fox', CAST('2024-01-01 00:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPTWBXJNAKK288S3D89JK7L', '124', '123', 'https://foo.bar/1239', 'Jumping over the lazy dog', CAST('2024-01-01 00:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPV1XJNAKK288S3D89JK7M1', '125', '127', 'https://baz.bat/1240', 'This little piggy', CAST('2024-01-01 00:00:00+00' AS TIMESTAMPTZ)),
    ('01JBPV1XJNAKK288S3D89JK7N2', '125', '127', 'https://baz.bat/1241', 'Pancakes > waffles', CAST('2024-01-01 00:00:00+00' AS TIMESTAMPTZ))
;
