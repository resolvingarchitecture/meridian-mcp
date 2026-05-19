use assert_fs::prelude::*;
use assert_fs::TempDir;
use meridian_mcp::scanner;

fn make_layered_project() -> TempDir {
    let dir = TempDir::new().unwrap();

    // Controllers
    dir.child("src/controllers/OrderController.ts")
        .write_str(
            r#"
import { OrderService } from '../services/OrderService';
export class OrderController {
  constructor(private svc: OrderService) {}
}
"#).unwrap();

    // Services
    dir.child("src/services/OrderService.ts")
        .write_str(
            r#"
import { OrderRepository } from '../repositories/OrderRepository';
import { Order } from '../domain/Order';
export class OrderService {
  constructor(private repo: OrderRepository) {}
}
"#).unwrap();

    // Domain — should not import from controllers or services
    dir.child("src/domain/Order.ts")
        .write_str(
            r#"
import { Money } from './Money';
export class Order {
  constructor(public id: string, public total: Money) {}
}
"#).unwrap();

    dir.child("src/domain/Money.ts")
        .write_str("export record Money(amount: number, currency: string) {}")
        .unwrap();

    // Repositories
    dir.child("src/repositories/OrderRepository.ts")
        .write_str(
            r#"
import { Order } from '../domain/Order';
import { Database } from '../infra/Database';
export class OrderRepository {
  constructor(private db: Database) {}
}
"#).unwrap();

    // Infra
    dir.child("src/infra/Database.ts")
        .write_str("export class Database { connect() {} }")
        .unwrap();

    // ADR
    dir.child("docs/adr/ADR-001-repository-pattern.md")
        .write_str("# ADR-001: Use repository pattern\n\nStatus: Accepted\n\n...")
        .unwrap();

    dir
}

#[test]
fn detects_layers_in_layered_project() {
    let project = make_layered_project();
    let model = scanner::scan(project.path()).unwrap();

    assert!(
        model.layers.iter().any(|l| l == "controllers"),
        "should detect controllers"
    );
    assert!(
        model.layers.iter().any(|l| l == "services"),
        "should detect services"
    );
    assert!(
        model.layers.iter().any(|l| l == "domain"),
        "should detect domain"
    );
    assert!(
        model.layers.iter().any(|l| l == "repositories"),
        "should detect repositories"
    );
}

#[test]
fn infers_layered_ddd_style() {
    let project = make_layered_project();
    let model = scanner::scan(project.path()).unwrap();
    assert_eq!(model.style, "layered_ddd");
}

#[test]
fn harvests_adrs() {
    let project = make_layered_project();
    let model = scanner::scan(project.path()).unwrap();
    assert!(!model.adrs.is_empty(), "should harvest at least one ADR");
    assert!(model.adrs[0].contains("ADR-001"));
}

#[test]
fn domain_appears_last_in_layer_order() {
    let project = make_layered_project();
    let model = scanner::scan(project.path()).unwrap();
    let domain_pos = model.layer_order.iter().position(|l| l == "domain");
    let ctrl_pos = model.layer_order.iter().position(|l| l == "controllers");
    if let (Some(d), Some(c)) = (domain_pos, ctrl_pos) {
        assert!(
            d > c,
            "domain should appear after controllers in layer order"
        );
    }
}

#[test]
fn handles_empty_project_gracefully() {
    let dir = TempDir::new().unwrap();
    let model = scanner::scan(dir.path()).unwrap();
    assert!(model.layers.is_empty());
    assert_eq!(model.style, "modular");
}

#[test]
fn handles_project_with_no_adrs() {
    let dir = TempDir::new().unwrap();
    dir.child("src/services/Foo.ts")
        .write_str("export class Foo {}")
        .unwrap();
    let model = scanner::scan(dir.path()).unwrap();
    // Should not panic — adrs can be empty
    assert!(model.adrs.len() < 10);
}
