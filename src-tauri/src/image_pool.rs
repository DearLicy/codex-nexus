use crate::models::{ImageJob, JobStatus};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagePoolMember {
    pub account_id: String,
    pub max_concurrent: u16,
    pub in_flight: u16,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageAssignment {
    pub job_id: String,
    pub account_id: String,
}

/// A small local scheduler for image requests. Image jobs have their own
/// concurrency slots and do not consume text-session affinity bindings.
#[derive(Clone, Debug, Default)]
pub struct ImagePool {
    members: HashMap<String, ImagePoolMember>,
    queue: VecDeque<ImageJob>,
}

impl ImagePool {
    pub fn new(members: impl IntoIterator<Item = ImagePoolMember>) -> Self {
        Self {
            members: members
                .into_iter()
                .map(|member| (member.account_id.clone(), member))
                .collect(),
            queue: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, mut job: ImageJob) -> Option<ImageAssignment> {
        job.status = JobStatus::Pending;
        self.queue.push_back(job);
        self.start_next()
    }

    pub fn start_next(&mut self) -> Option<ImageAssignment> {
        let index = self
            .queue
            .iter()
            .enumerate()
            .filter_map(|(index, job)| {
                let account = self
                    .members
                    .values()
                    .filter(|member| {
                        member.enabled && member.in_flight < member.max_concurrent.max(1)
                    })
                    .min_by_key(|member| member.in_flight)?;
                Some((index, account.account_id.clone(), job.id.clone()))
            })
            .next();
        let (index, account_id, job_id) = index?;
        self.queue.remove(index);
        if let Some(member) = self.members.get_mut(&account_id) {
            member.in_flight = member.in_flight.saturating_add(1);
        }
        Some(ImageAssignment { job_id, account_id })
    }

    pub fn complete(&mut self, account_id: &str) -> Option<ImageAssignment> {
        if let Some(member) = self.members.get_mut(account_id) {
            member.in_flight = member.in_flight.saturating_sub(1);
        }
        self.start_next()
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    pub fn in_flight(&self, account_id: &str) -> Option<u16> {
        self.members.get(account_id).map(|member| member.in_flight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ImageJob;

    #[test]
    fn image_jobs_use_independent_slots_and_queue() {
        let mut pool = ImagePool::new([
            ImagePoolMember {
                account_id: "a".into(),
                max_concurrent: 1,
                in_flight: 0,
                enabled: true,
            },
            ImagePoolMember {
                account_id: "b".into(),
                max_concurrent: 1,
                in_flight: 0,
                enabled: true,
            },
        ]);
        let first = pool.enqueue(ImageJob::new("j1", "one", 0)).unwrap();
        let second = pool.enqueue(ImageJob::new("j2", "two", 0)).unwrap();
        assert_ne!(first.account_id, second.account_id);
        assert!(pool.enqueue(ImageJob::new("j3", "three", 0)).is_none());
        assert_eq!(pool.queued(), 1);
        assert!(pool.complete(&first.account_id).is_some());
    }
}
