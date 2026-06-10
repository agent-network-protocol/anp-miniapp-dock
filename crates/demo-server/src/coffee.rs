use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drink {
    pub id: String,
    pub name: String,
    pub price: u32,
    pub image: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrinksResponse {
    pub drinks: Vec<Drink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmOrderRequest {
    pub drink_id: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub sugar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayOrderRequest {
    pub order_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub order_id: String,
    pub drink_id: String,
    pub drink_name: String,
    pub payable: u32,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    PendingPayment,
    Paid,
}

#[derive(Debug, Clone)]
struct CoffeeData {
    drinks: Vec<Drink>,
    orders: BTreeMap<String, Order>,
    next_order: u64,
}

#[derive(Debug, Clone)]
pub struct CoffeeStore {
    data: Arc<Mutex<CoffeeData>>,
}

impl Default for CoffeeStore {
    fn default() -> Self {
        Self {
            data: Arc::new(Mutex::new(CoffeeData {
                drinks: vec![
                    Drink {
                        id: "latte".to_owned(),
                        name: "Latte".to_owned(),
                        price: 18,
                        image: "https://img.example/latte.png".to_owned(),
                    },
                    Drink {
                        id: "americano".to_owned(),
                        name: "Americano".to_owned(),
                        price: 15,
                        image: "https://img.example/americano.png".to_owned(),
                    },
                    Drink {
                        id: "mocha".to_owned(),
                        name: "Mocha".to_owned(),
                        price: 20,
                        image: "https://img.example/mocha.png".to_owned(),
                    },
                ],
                orders: BTreeMap::new(),
                next_order: 1,
            })),
        }
    }
}

impl CoffeeStore {
    pub fn search_drinks(&self, query: Option<&str>) -> DrinksResponse {
        let query = query.unwrap_or_default().to_ascii_lowercase();
        let drinks = self
            .data
            .lock()
            .map(|data| {
                data.drinks
                    .iter()
                    .filter(|drink| {
                        query.is_empty()
                            || drink.id.contains(&query)
                            || drink.name.to_ascii_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        DrinksResponse { drinks }
    }

    pub fn confirm_order(&self, request: ConfirmOrderRequest) -> Result<Order, CoffeeError> {
        let mut data = self.data.lock().map_err(|_| CoffeeError::Unavailable)?;
        let drink = data
            .drinks
            .iter()
            .find(|drink| drink.id == request.drink_id)
            .cloned()
            .ok_or(CoffeeError::UnknownDrink)?;
        let order_id = format!("order_demo_{:03}", data.next_order);
        data.next_order += 1;
        let order = Order {
            order_id: order_id.clone(),
            drink_id: drink.id,
            drink_name: drink.name,
            payable: drink.price,
            status: OrderStatus::PendingPayment,
        };
        data.orders.insert(order_id, order.clone());
        Ok(order)
    }

    pub fn pay_order(&self, request: PayOrderRequest) -> Result<Order, CoffeeError> {
        let mut data = self.data.lock().map_err(|_| CoffeeError::Unavailable)?;
        let order = data
            .orders
            .get_mut(&request.order_id)
            .ok_or(CoffeeError::UnknownOrder)?;
        order.status = OrderStatus::Paid;
        Ok(order.clone())
    }

    pub fn get_order(&self, order_id: &str) -> Result<Order, CoffeeError> {
        self.data
            .lock()
            .map_err(|_| CoffeeError::Unavailable)?
            .orders
            .get(order_id)
            .cloned()
            .ok_or(CoffeeError::UnknownOrder)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoffeeError {
    UnknownDrink,
    UnknownOrder,
    Unavailable,
}
