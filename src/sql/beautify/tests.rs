use super::beautify;

#[test]
fn one_item_and_condition_per_line() {
    let sql = "select iet.MandatorsID, sj.Description as 'Job, description', count(x, y) from SCHEDULERJOBS sj, IMPORTEXPORTTEMPLATES iet \
               where sj.Active = 'Y' and sj.ID = iet.ID and iet.Type between 1 and 3 or iet.X in (select a from b where c = 1 and d = 2)";
    let want = "\
SELECT
  iet.MandatorsID,
  sj.Description AS 'Job, description',
  COUNT(x, y)
FROM
  SCHEDULERJOBS sj,
  IMPORTEXPORTTEMPLATES iet
WHERE
  sj.Active = 'Y'
  AND sj.ID = iet.ID
  AND iet.Type BETWEEN 1 AND 3
  OR iet.X IN (
    SELECT
      a
    FROM
      b
    WHERE
      c = 1
      AND d = 2
  );";
    assert_eq!(beautify(sql), want);
}

#[test]
fn joins_top_and_order() {
    let sql = "SELECT TOP 5 a.id, CASE WHEN a.x = 1 AND a.y = 2 THEN 'y' END FROM a LEFT JOIN b ON b.id = a.id AND b.ok = 1 ORDER BY a.id DESC, a.n LIMIT 10";
    let want = "\
SELECT TOP 5
  a.id,
  CASE
    WHEN a.x = 1 AND a.y = 2 THEN 'y'
  END
FROM
  a
  LEFT JOIN b ON b.id = a.id
  AND b.ok = 1
ORDER BY
  a.id DESC,
  a.n
LIMIT 10;";
    assert_eq!(beautify(sql), want);
}

#[test]
fn ctes_and_case_get_their_own_blocks() {
    let sql = "WITH source AS ( SELECT t.ID, CASE WHEN a = 'R' THEN t.V WHEN b = 1 AND coalesce(c, 0) <> 0 THEN x ELSE y END VehiclesID, t.M \
               FROM dbo.P t LEFT JOIN dbo.c c ON c.ID = t.CID WHERE t.Q > 0 ) \
               SELECT s.ID, LEAD(M) OVER ( PARTITION BY v ORDER BY T ) AS N FROM source s";
    let want = "\
WITH source AS (
  SELECT
    t.ID,
    CASE
      WHEN a = 'R' THEN t.V
      WHEN b = 1 AND coalesce(c, 0) <> 0 THEN x
      ELSE y
    END VehiclesID,
    t.M
  FROM
    dbo.P t
    LEFT JOIN dbo.c c ON c.ID = t.CID
  WHERE
    t.Q > 0
)
SELECT
  s.ID,
  LEAD(M) OVER ( PARTITION BY v ORDER BY T ) AS N
FROM
  source s;";
    assert_eq!(beautify(sql), want);
}
