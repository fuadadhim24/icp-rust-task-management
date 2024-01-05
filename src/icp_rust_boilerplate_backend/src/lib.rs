#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Task {
    id: u64,
    title: String,
    description: String,
    assigned_to: Option<String>,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Task {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Task {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static TASKS: RefCell<StableBTreeMap<u64, Task, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct TaskPayload {
    title: String,
    description: String,
    assigned_to: Option<String>,
}

#[ic_cdk::query]
fn get_task(id: u64) -> Result<Task, Error> {
    match _get_task(&id) {
        Some(task) => Ok(task),
        None => Err(Error::NotFound {
            msg: format!("a task with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_task(task: TaskPayload) -> Result<Task, Error> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let task = Task {
        id,
        title: task.title,
        description: task.description,
        assigned_to: task.assigned_to,
        created_at: time(),
        updated_at: None,
    };
    do_insert(&task);
    Ok(task)
 }

#[ic_cdk::update]
fn update_task(id: u64, payload: TaskPayload) -> Result<Task, Error> {
    match TASKS.with(|service| service.borrow().get(&id)) {
        Some(mut task) => {
            task.title = payload.title;
            task.description = payload.description;
            task.assigned_to = payload.assigned_to;
            task.updated_at = Some(time());
            do_insert(&task);
            Ok(task)
        }
        None => Err(Error::NotFound {
            msg: format!("couldn't update a task with id={}. task not found", id),
        }),
    }
}

fn do_insert(task: &Task) {
    TASKS.with(|service| service.borrow_mut().insert(task.id, task.clone()));
}

#[ic_cdk::update]
fn delete_task(id: u64) -> Result<Task, Error> {
    match TASKS.with(|service| service.borrow_mut().remove(&id)) {
        Some(task) => Ok(task),
        None => Err(Error::NotFound {
            msg: format!("couldn't delete a task with id={}. task not found.", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// a helper method to get a task by id. used in get_task/update_task
fn _get_task(id: &u64) -> Option<Task> {
    TASKS.with(|service| service.borrow().get(id))
}

// need this to generate candid
ic_cdk::export_candid!();
