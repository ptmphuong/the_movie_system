use crate::auth::{hasher, verify_pass};

use crate::err_info;
use crate::error_handling::{Result, WarpRejections};
use shared_stuff::db_structs::{
    DBGroup, DBGroupStruct, DBUser, DBUserStruct, GroupData, SystemState, UserData,
};
use shared_stuff::groups_stuff::{GroupForm, GroupInfo};
use shared_stuff::UserInfo;
use shared_stuff::YewMovieDisplay;
use sqlx::pool::PoolConnection;
use sqlx::Sqlite;
use sqlx::{query, query_as, SqlitePool};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;
use warp::reject::custom;

pub async fn db_verify_group_member(
    group_id: String,
    username: String,
    db: &SqlitePool,
) -> Result<DBGroupStruct> {
    let group_struct = db_get_group(db, &group_id).await?;
    let members = &group_struct.group_data.members;
    if members.contains(&username) {
        Ok(group_struct)
    } else {
        Err(custom(WarpRejections::UserNotInGroup(err_info!())))
    }
}

// Need to make sure the new member is registered, and there are no duplicates in group.
pub async fn db_add_user_to_group(group_id: &str, new_member: &str, db: &SqlitePool) -> Result<()> {
    let mut group_struct = db_get_group(db, group_id).await?;
    let mut user_struct = db_get_user(db, &new_member).await?;
    group_struct
        .group_data
        .members
        .push_back(new_member.to_string());

    db_update_group(db, &group_struct).await?;
    log::info!("group_data members updated");

    let group_uuid =
        Uuid::parse_str(&group_id).map_err(|_e| custom(WarpRejections::UuidError(err_info!())))?;
    let group_info = GroupInfo {
        uuid: group_uuid.to_string(),
        name: group_struct.group_data.group_name.clone(),
    };
    user_struct.user_data.groups.insert(group_info);
    db_update_user(db, user_struct).await?;
    log::info!("user group_info updated");
    Ok(())
}

pub async fn create_user_data(input: UserInfo) -> Result<UserData> {
    let id = Uuid::new_v4();
    let (hashed_password, salt) = hasher(&input.password).await?;
    let groups = HashSet::new();
    let now = chrono::Utc::now().timestamp();
    Ok(UserData {
        id,
        hashed_password,
        salt,
        groups,
        date_created: now,
        date_modified: now,
    })
}

pub fn create_group_data(input: GroupForm) -> GroupData {
    let mut members_vec = VecDeque::new();
    members_vec.push_back(input.username);
    let now = chrono::Utc::now().timestamp();
    let turn = String::from("");
    GroupData {
        group_name: input.group_name,
        members: members_vec,
        movies_watched: HashSet::new(),
        current_movies: HashSet::new(),
        ready_status: HashMap::new(),
        system_state: SystemState::AddingMovies,
        turn,
        date_created: now,
        date_modified: now,
    }
}

pub async fn acquire_db(db: &SqlitePool) -> Result<PoolConnection<Sqlite>> {
    let conn = db
        .acquire()
        .await
        .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;
    Ok(conn)
}

pub async fn db_get_user(db: &SqlitePool, username: &str) -> Result<DBUserStruct> {
    let mut conn = acquire_db(db).await?;
    log::info!("inside db_get_user");
    let db_user = query_as!(
        DBUser,
        r#"
            select *
            from users
            where username = $1
        "#,
        username
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;

    let user_struct = db_get_user_data(db_user)?;

    Ok(user_struct)
}

pub fn db_get_user_data(db_user: DBUser) -> Result<DBUserStruct> {
    log::info!("inside db_get_user_data");
    log::info!("DBUser is: {:?}", &db_user);
    log::info!("DBUser data is: {:?}", &db_user.data);
    let user_data: UserData = serde_json::from_str(&db_user.data)
        .map_err(|_| custom(WarpRejections::SerializationError(err_info!())))?;
    let user_struct = DBUserStruct {
        username: db_user.username,
        user_data,
    };
    Ok(user_struct)
}

pub async fn db_insert_user(db: &SqlitePool, username: &str, user_data: UserData) -> Result<()> {
    let mut conn = acquire_db(db).await?;
    let serialized_user_data = serde_json::to_string(&user_data).expect("serialization error");
    query!(
        r#"
            insert into users (username, data)
            values ($1, $2);
        "#,
        username,
        serialized_user_data,
    )
    .execute(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;

    Ok(())
}

pub async fn db_update_user(db: &SqlitePool, user_struct: DBUserStruct) -> Result<()> {
    let mut conn = acquire_db(db).await?;
    let serialized_user_data =
        serde_json::to_string(&user_struct.user_data).expect("serialization error");
    query!(
        r#"
            update users set data=$1 where username=$2
        "#,
        serialized_user_data,
        user_struct.username,
    )
    .execute(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;

    Ok(())
}

pub async fn db_delete_user(db: &SqlitePool, username: &str) -> Result<()> {
    let mut conn = acquire_db(db).await?;
    query!(
        r#"
                    delete from users
                    WHERE username = $1;
                    "#,
        username
    )
    .execute(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;
    Ok(())
}

pub async fn db_get_group(db: &SqlitePool, group_id: &str) -> Result<DBGroupStruct> {
    log::info!("inside db_get_group. group_id is: {:?}", &group_id);
    let mut conn = acquire_db(db).await?;
    let db_group = query_as!(
        DBGroup,
        r#"
select *
from groups
where id = $1
"#,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;

    let group_data = db_get_group_data(db_group)?;
    Ok(group_data)
}

//pub async fn db_get_group(db: &SqlitePool, group_id: &str) -> Result<GroupData> {
//log::info!("inside db_get_group1. group_id is: {:?}", &group_id);
//let mut conn = acquire_db(db).await?;
//let db_group = query_as!(
//DBGroup,
//r#"
//select *
//from groups
//where id = $1
//"#,
//group_id
//)
//.fetch_one(&mut conn)
//.await;

//match db_group {
//Ok(db_group) => {
//let group_data: GroupData = serde_json::from_str(&db_group.data)
//.map_err(|_| custom(WarpRejections::SerializationError(err_info!())))?;
//Ok(group_data)
//}
//Err(_) => {
//log::info!("Cannot find group_id {} in db", &group_id);
//Err(custom(WarpRejections::GroupNotExist(err_info!())))?
//}
//}
//}

pub async fn db_update_group(db: &SqlitePool, group_struct: &DBGroupStruct) -> Result<()> {
    let mut conn = acquire_db(db).await?;
    let serialized_group_data =
        serde_json::to_string(&group_struct.group_data).expect("serialization error");
    query!(
        r#"
            update groups set data=$1 where id=$2
        "#,
        serialized_group_data,
        group_struct.id,
    )
    .execute(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;

    Ok(())
}

pub async fn db_user_leave_group1(db: &SqlitePool, username: String, group_id: &str) -> Result<()> {
    let mut group_struct = db_get_group(db, group_id).await?;
    let group_name = &group_struct.group_data.group_name;
    let group_members = group_struct.group_data.members;
    let mut user_struct = db_get_user(db, &username).await?;
    let user_groups = user_struct.user_data.groups;

    let user_groups = user_groups
        .iter()
        .filter(|group_info| !group_info.name.eq(group_name))
        .cloned()
        .collect::<HashSet<GroupInfo>>();
    user_struct.user_data.groups = user_groups;
    db_update_user(db, user_struct).await?;
    log::info!("db_user_leave_group1 - User GroupInfo updated");

    let updated_group_members = group_members
        .iter()
        .filter(|name| !username.eq(*name))
        .map(|name| name.to_owned())
        .collect::<VecDeque<String>>();
    log::info!("group_members is: {:?}", &group_members);
    group_struct.group_data.members = updated_group_members.clone();

    match updated_group_members.is_empty() {
        true => {
            log::info!("no members. deleting group");
            db_delete_group(db, &group_id).await?
        }
        false => {
            log::info!("updating group info");
            //let group_struct = DBGroupStruct {
            //id: group_id.to_string(),
            //group_data,
            //};
            db_update_group(db, &group_struct).await?
        }
    }
    log::info!("db_user_leave_group1 - GroupData updated");

    Ok(())
}

pub fn db_get_group_data(db_group: DBGroup) -> Result<DBGroupStruct> {
    log::info!("DBGroup is: {:?}", &db_group);
    let group_data: GroupData = serde_json::from_str(&db_group.data)
        .map_err(|_| custom(WarpRejections::SerializationError(err_info!())))?;
    log::info!("group_data is: {:?}", &group_data);
    let group_struct = DBGroupStruct {
        id: db_group.id,
        group_data,
    };
    Ok(group_struct)
}

pub async fn db_insert_group(db: &SqlitePool, group_struct: DBGroupStruct) -> Result<()> {
    let mut conn = acquire_db(db).await?;
    let serialized_group_data =
        serde_json::to_string(&group_struct.group_data).expect("serialization error");
    query!(
        r#"
            insert into groups (id, data)
            values ($1, $2);
        "#,
        group_struct.id,
        serialized_group_data,
    )
    .execute(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;

    Ok(())
}

pub async fn db_delete_group(db: &SqlitePool, group_id: &str) -> Result<()> {
    let mut conn = acquire_db(db).await?;
    query!(
        r#"
                    delete from groups
                    WHERE id = $1;
                    "#,
        group_id
    )
    .execute(&mut conn)
    .await
    .map_err(|_| custom(WarpRejections::SqlxError(err_info!())))?;
    Ok(())
}

pub async fn db_get_group_members(db: &SqlitePool, group_id: &str) -> Result<VecDeque<String>> {
    log::info!("inside db_get_group_members");
    let group_struct = db_get_group(db, group_id).await?;
    let members = group_struct.group_data.members;
    log::info!("members are: {:?}", &members);
    Ok(members)
}

//pub async fn db_get_user_groups(db: &SqlitePool, username: &str) -> Result<HashSet<GroupInfo>> {
//let user_struct = db_get_user(db, username).await?;
//let user_groups = user_struct.user_data.groups;
//Ok(user_groups)
//}

//pub async fn db_get_group_id(db: &SqlitePool, group_name: &str, username: &str) -> Result<String> {
//log::info!(
//"db_get_group_id group_name: {:?}, username: {:?}",
//&group_name,
//&username
//);
//let db_user_data = db_get_user(db, username).await?;
//let user_groups = db_user_data.1.groups;
//let option_group_info = user_groups.iter().find(|group_info| {
//log::info!("name: {:?}, uuid: {:?}", &group_info.name, &group_info.uuid);
//group_info.name.as_str() == group_name
//});
//log::info!("option_id: {:?}", &option_group_info);
//if let Some(info) = option_group_info {
//Ok(info.uuid.clone())
//} else {
//Err(custom(WarpRejections::SqlxError(err_info!())))
//}
//}

//pub async fn db_add_user_to_group(db: &SqlitePool, add_user: &AddUser) -> Result<()> {
//let group_id = db_get_group_id(db, &add_user.group_name, &add_user.username).await?;
//let mut group_struct = db_get_group(db, &group_id).await?;
//let mut db_group_members = group_struct.group_data.members;
//db_group_members.push_back(add_user.new_member.to_string());
//log::info!("db_group_members: {:?}", &db_group_members);
//group_struct.group_data.members = db_group_members;
//db_update_group(db, &group_struct).await?;
//let mut user_info = db_get_user(db, &add_user.new_member).await?;
//let group_uuid =
//Uuid::parse_str(&group_id).map_err(|_e| custom(WarpRejections::UuidError(err_info!())))?;
//let group_info = GroupInfo {
//uuid: group_uuid.to_string(),
//name: add_user.group_name.clone(),
//};
//user_info.1.groups.insert(group_info);
//db_update_user(db, user_info).await?;
//Ok(())
//}

pub async fn db_save_group_movies(db: &SqlitePool, db_struct: &DBGroupStruct) -> Result<()> {
    log::info!("db_struct: {:?}", &db_struct);
    db_update_group(db, db_struct).await?;

    Ok(())
}

//pub async fn db_get_group_movies(
//db: &SqlitePool,
//group_form: &GroupForm,
//) -> Result<HashSet<YewMovieDisplay>> {
//let username = &group_form.username;
//let group_name = &group_form.group_name;
//let group_id = db_get_group_id(db, group_name, username).await?;
//let group_data: GroupData = db_get_group(db, &group_id).await?;
//Ok(group_data.current_movies)
//}

pub async fn db_add_group_to_user(
    db: &SqlitePool,
    mut user_struct: DBUserStruct,
    group: GroupInfo,
) -> Result<()> {
    user_struct.user_data.groups.insert(group);
    //let mut new_groups = user_data.1.groups;
    //new_groups.insert(group);
    //user_data.1.groups = new_groups;
    db_update_user(db, user_struct).await?;
    Ok(())
}

//pub async fn db_group_add_new_user(db: &SqlitePool, user_struct: &AddUser) -> Result<()> {
//let username = &user_struct.username;
//let new_member = &user_struct.new_member;
//let group_name = &user_struct.group_name;
//let group_id = db_get_group_id(db, group_name, username).await?;
//let group_uuid =
//Uuid::parse_str(&group_id).map_err(|_e| custom(WarpRejections::UuidError(err_info!())))?;

//match db_get_user(db, new_member).await {
//Ok(user_data) => {
//log::info!("in here");
//let group_info = GroupInfo {
//uuid: group_uuid.to_string(),
//name: group_name.clone(),
//};
//db_add_group_to_user(db, user_data.clone(), group_info).await?;
//db_add_user_to_group(db, user_struct).await?;
//}
//Err(e) => return Err(e),
//}

//Ok(())
//}

//pub async fn db_user_leave_group(db: &SqlitePool, group_form: &GroupForm) -> Result<()> {
//log::info!("group_form is: {:?}", &group_form,);
//let username = &group_form.username;
//let group_name = &group_form.group_name;
//let groups = db_get_user_groups(db, username).await?;
//log::info!("groups are: {:?}", &groups);

//let group_id = db_get_group_id(db, group_name, username).await?;
//log::info!("group_id is: {:?}", &group_id);
//let mut group_data = db_get_group(db, &group_id).await?;
//let mut user_data = db_get_user(db, username).await?;
//log::info!("user_data is: {:?}", &user_data);
//let user_groups = db_get_user_groups(db, username)
//.await?
//.iter()
//.filter(|group_info| !group_info.name.eq(group_name))
//.cloned()
//.collect::<HashSet<GroupInfo>>();
//log::info!("user_groups is: {:?}", &user_groups);
//user_data.1.groups = user_groups;

//db_update_user(db, user_data).await?;

//let group_members = db_get_group_members(db, &group_id)
//.await?
//.iter()
//.filter(|name| *name != username)
//.map(|name| name.to_owned())
//.collect::<VecDeque<String>>();
//log::info!("group_members is: {:?}", &group_members);
//group_data.members = group_members.clone();

//match group_members.is_empty() {
//true => {
//log::info!("inside true");
//db_delete_group(db, &group_id).await?
//}
//false => {
//log::info!("inside false");
//let group_struct = DBGroupStruct {
//id: group_id,
//group_data,
//};
//db_update_group(db, &group_struct).await?
//}
//}

//Ok(())
//}

pub async fn db_update_password(
    db: &SqlitePool,
    old_user: &UserInfo,
    new_user: &UserInfo,
) -> Result<()> {
    let now = sqlx::types::chrono::Utc::now().timestamp();

    match db_get_user(db, &old_user.username).await {
        Ok(mut old_user_struct) => match verify_pass(
            old_user.password.clone(),
            old_user_struct.user_data.salt,
            old_user_struct.user_data.hashed_password,
        )? {
            true => {
                let (new_hashed_password, new_salt) = hasher(&new_user.password).await?;
                old_user_struct.user_data.salt = new_salt;
                old_user_struct.user_data.hashed_password = new_hashed_password;
                old_user_struct.user_data.date_modified = now;
                db_update_user(db, old_user_struct).await?;
            }
            false => {
                custom(WarpRejections::AuthError(err_info!()));
            }
        },
        Err(_) => return Err(custom(WarpRejections::SqlxError(err_info!()))),
    }
    Ok(())
}

pub async fn db_update_username(
    db: &SqlitePool,
    old_user: &DBUserStruct,
    old_password: String,
) -> Result<()> {
    let now = sqlx::types::chrono::Utc::now().timestamp();
    match db_get_user(db, &old_user.username).await {
        Ok(mut user_struct) => match verify_pass(
            old_password,
            user_struct.user_data.salt.clone(),
            user_struct.user_data.hashed_password.clone(),
        )? {
            true => {
                user_struct.user_data.date_modified = now;
                db_update_user(db, user_struct).await?;
            }
            false => {
                custom(WarpRejections::AuthError(err_info!()));
            }
        },
        Err(_) => return Err(custom(WarpRejections::SqlxError(err_info!()))),
    }
    Ok(())
}
