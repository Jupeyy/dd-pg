use base_log::log::SystemLog;
use sql::database::{Database, DatabaseDetails};
use sqlx::{Acquire, FromRow};

pub struct DdRaceDatabase {
    db: Database,
}

impl DdRaceDatabase {
    pub fn new(_log: &SystemLog) -> anyhow::Result<Self> {
        let connection_details = DatabaseDetails {
            host: "localhost".to_string(),
            port: 3306,
            database: "ddnet".to_string(),
            username: "ddnet".to_string(),
            password: "TODO:".to_string(),
            thread_count: 3,
        };
        let db = Database::new(connection_details)?;
        Ok(Self { db })
    }

    pub fn top_5(&mut self, offset: i32) {
        let limit_start = (offset.abs() - 1).max(0);
        let order = if offset >= 0 { "ASC" } else { "DESC" };

        // check sort method
        let sql_str = format!(
            "
            SELECT Name, Time, Ranking 
            FROM (
            SELECT RANK() OVER w AS Ranking, MIN(Time) AS Time, Name 
            FROM record_race 
            WHERE Map = ? 
            AND Server LIKE ? 
            GROUP BY Name 
            WINDOW w AS (ORDER BY MIN(Time))
            ) as a 
            ORDER BY Ranking {}
            LIMIT {}, ?
        ",
            order, limit_start
        );

        #[derive(FromRow)]
        struct Test {
            name: String,
            time: f32,
            rank: i32,
        }
        let pool = self.db.pool.clone();
        self.db.queue_task::<Vec<String>, _>(async move {
            let mut connection = pool.acquire().await?;
            let connection = connection.acquire().await?;
            let query = Database::get_query::<Test>(&sql_str)
                .bind("TODO: map_name")
                .bind("%")
                .bind(5);
            let results = query.fetch_all(connection).await?;
            let mut res: Vec<String> = Default::default();
            res.push("------------ Global Top ------------".to_string());
            res.append(
                &mut results
                    .iter()
                    .map(|row| format!("{}. {} Time: {}", row.rank, row.name, row.time))
                    .collect(),
            );
            Ok(res)
        });

        // show top
        /*
        char aServerLike[16];
        str_format(aServerLike, sizeof(aServerLike), "%%%s%%", pData->m_aServer);

        if(pSqlServer->PrepareStatement(aBuf, pError, ErrorSize))
        {
            return true;
        }
        pSqlServer->BindString(1, pData->m_aMap);
        pSqlServer->BindString(2, aServerLike);
        pSqlServer->BindInt(3, 3);

        str_format(pResult->m_Data.m_aaMessages[Line], sizeof(pResult->m_Data.m_aaMessages[Line]),
            "------------ %s Top ------------", pData->m_aServer);
        Line++;

        // show top
        while(!pSqlServer->Step(&End, pError, ErrorSize) && !End)
        {
            char aName[MAX_NAME_LENGTH];
            pSqlServer->GetString(1, aName, sizeof(aName));
            float Time = pSqlServer->GetFloat(2);
            str_time_float(Time, TIME_HOURS_CENTISECS, aTime, sizeof(aTime));
            int Rank = pSqlServer->GetInt(3);
            str_format(pResult->m_Data.m_aaMessages[Line], sizeof(pResult->m_Data.m_aaMessages[Line]),
                "%d. %s Time: %s", Rank, aName, aTime);
            Line++;
        }

        return !End;*/
    }
}
